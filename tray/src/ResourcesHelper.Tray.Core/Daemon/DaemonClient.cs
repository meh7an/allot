using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.Daemon;

/// <summary>
/// HTTP client for the daemon routes the tray app uses. It never starts rhd by itself; that is
/// the user's Start service, through <see cref="ServiceLauncher"/>.
/// </summary>
/// <param name="http">Client with its base address set to the daemon; disposed with this
/// one. Each request sets its own timeout.</param>
/// <param name="tokenFile">The bearer token file, read on first use and after a 401.</param>
public sealed class DaemonClient(HttpClient http, string tokenFile) : IDisposable, IJobStopper
{
    private static readonly TimeSpan ConnectTimeout = TimeSpan.FromSeconds(1);
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(3);

    // rhd answers a kill after up to three sweeps over the tree, each waiting for the processes
    // it terminated.
    private static readonly TimeSpan StopTimeout = TimeSpan.FromSeconds(10);

    private string? _token;

    /// <summary>
    /// A client for the local install: the port from <c>config.toml</c>, the token file beside it.
    /// </summary>
    public static DaemonClient ForInstall(DataDirectory data)
    {
        var port = DaemonConfig.ReadPort(data).ToString(CultureInfo.InvariantCulture);
        var handler = new SocketsHttpHandler { ConnectTimeout = ConnectTimeout, UseProxy = false };
        var client = new HttpClient(handler)
        {
            BaseAddress = new Uri($"http://127.0.0.1:{port}/"),
            Timeout = Timeout.InfiniteTimeSpan,
        };
        return new DaemonClient(client, data.TokenFile);
    }

    /// <summary>
    /// <c>GET /ui/state</c>, mapped to a service status. Only cancellation throws.
    /// </summary>
    public async Task<PollResult> PollAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var response = await SendWithTokenAsync(
                    HttpMethod.Get, "ui/state", RequestTimeout, cancellationToken)
                .ConfigureAwait(false);
            if (response is null)
            {
                return new PollResult(ServiceStatus.Stopped, Detail: $"no token in {tokenFile}");
            }

            return response.StatusCode switch
            {
                HttpStatusCode.OK => await DecodeAsync(response, cancellationToken)
                    .ConfigureAwait(false),
                HttpStatusCode.NotFound => await WithoutUiStateAsync(cancellationToken)
                    .ConfigureAwait(false),
                HttpStatusCode.ServiceUnavailable => new PollResult(
                    ServiceStatus.Starting,
                    Detail: "the daemon has no first sample yet"),
                HttpStatusCode.Unauthorized => new PollResult(
                    ServiceStatus.Incompatible,
                    Detail: "the daemon refused the token"),
                _ => new PollResult(
                    ServiceStatus.Incompatible,
                    Detail: $"GET /ui/state answered {(int)response.StatusCode}"),
            };
        }
        catch (Exception error) when (IsUnreachable(error, cancellationToken))
        {
            return new PollResult(ServiceStatus.Stopped, Detail: error.Message);
        }
    }

    /// <summary>
    /// <c>GET /health</c>, which needs no token; <c>null</c> when nothing healthy answers.
    /// </summary>
    public async Task<HealthResponse?> HealthAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, "health");
            using var response = await SendAsync(request, RequestTimeout, cancellationToken)
                .ConfigureAwait(false);
            if (response.StatusCode != HttpStatusCode.OK)
            {
                return null;
            }

            var stream = await response.Content.ReadAsStreamAsync(cancellationToken)
                .ConfigureAwait(false);
            await using (stream.ConfigureAwait(false))
            {
                return await JsonSerializer.DeserializeAsync(
                        stream,
                        ContractJsonContext.Default.HealthResponse,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
        }
        catch (Exception error) when (IsUnreachable(error, cancellationToken)
            || error is JsonException)
        {
            return null;
        }
    }

    /// <summary>
    /// <c>POST /shutdown</c>: rhd answers 202, then stops gracefully. Only cancellation throws.
    /// </summary>
    /// <returns><c>null</c> when rhd accepted, else why not.</returns>
    public async Task<string?> ShutdownAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var response = await SendWithTokenAsync(
                    HttpMethod.Post, "shutdown", RequestTimeout, cancellationToken)
                .ConfigureAwait(false);
            return response switch
            {
                null => $"no token in {tokenFile}",
                { StatusCode: HttpStatusCode.Accepted } => null,
                _ => $"POST /shutdown answered {(int)response.StatusCode}",
            };
        }
        catch (Exception error) when (IsUnreachable(error, cancellationToken))
        {
            return error.Message;
        }
    }

    public Task<StopResult> KillAsync(long lease, CancellationToken cancellationToken) =>
        StopAsync(
            $"leases/{lease}/kill",
            ContractJsonContext.Default.KillResponse,
            StopResult.Killed,
            cancellationToken);

    public Task<StopResult> RemoveAsync(long lease, CancellationToken cancellationToken) =>
        StopAsync(
            $"leases/{lease}/remove",
            ContractJsonContext.Default.RemoveResponse,
            StopResult.Removed,
            cancellationToken);

    public void Dispose() => http.Dispose();

    /// <summary>
    /// The reason in an error body of the kill or remove route, for a card that already names
    /// the job: rhd's text starts with the lease.
    /// </summary>
    public static string ConflictReason(string body)
    {
        const string notKilled = " was not killed: ";
        var cut = body.IndexOf(notKilled, StringComparison.Ordinal);
        if (cut >= 0)
        {
            return body[(cut + notKilled.Length)..];
        }

        if (body.Contains(" has already ended", StringComparison.Ordinal))
        {
            return "the job has already ended";
        }

        return body.Length == 0 ? "the service refused" : body;
    }

    // A timeout surfaces as a cancellation that the caller did not ask for.
    private static bool IsUnreachable(Exception error, CancellationToken cancellationToken) =>
        error is HttpRequestException
        || (error is OperationCanceledException && !cancellationToken.IsCancellationRequested);

    private static async Task<PollResult> DecodeAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        var stream = await response.Content.ReadAsStreamAsync(cancellationToken)
            .ConfigureAwait(false);
        await using (stream.ConfigureAwait(false))
        {
            try
            {
                var state = await JsonSerializer.DeserializeAsync(
                        stream,
                        ContractJsonContext.Default.UiState,
                        cancellationToken)
                    .ConfigureAwait(false);
                return state is null
                    ? new PollResult(ServiceStatus.Incompatible, Detail: "empty UI state")
                    : new PollResult(ServiceStatus.Running, state, state.Service.Version);
            }
            catch (JsonException error)
            {
                return new PollResult(ServiceStatus.Incompatible, Detail: error.Message);
            }
        }
    }

    private async Task<StopResult> StopAsync<T>(
        string path,
        JsonTypeInfo<T> type,
        Func<T, StopResult> ended,
        CancellationToken cancellationToken)
    {
        try
        {
            using var response = await SendWithTokenAsync(
                    HttpMethod.Post, path, StopTimeout, cancellationToken)
                .ConfigureAwait(false);
            if (response is null)
            {
                return StopResult.Refused("the service's token file is missing");
            }

            if (response.StatusCode == HttpStatusCode.OK)
            {
                var stream = await response.Content.ReadAsStreamAsync(cancellationToken)
                    .ConfigureAwait(false);
                await using (stream.ConfigureAwait(false))
                {
                    var value = await JsonSerializer.DeserializeAsync(
                            stream, type, cancellationToken)
                        .ConfigureAwait(false);
                    return value is null
                        ? StopResult.Refused("the service sent an empty answer")
                        : ended(value);
                }
            }

            var body = (await response.Content.ReadAsStringAsync(cancellationToken)
                .ConfigureAwait(false)).Trim();
            return StopResult.Refused(response.StatusCode switch
            {
                HttpStatusCode.Conflict => ConflictReason(body),
                HttpStatusCode.NotFound => "the job no longer exists",
                HttpStatusCode.ServiceUnavailable => "the service has just started; try again",
                HttpStatusCode.Unauthorized => "the service refused the token",
                _ => $"the service answered {(int)response.StatusCode}",
            });
        }
        catch (JsonException)
        {
            return StopResult.Refused("the service's answer could not be read");
        }
        catch (Exception error) when (IsUnreachable(error, cancellationToken))
        {
            return StopResult.Refused(error is HttpRequestException
                ? "the service is not running"
                : $"the service did not answer within {StopTimeout.TotalSeconds:0} s");
        }
    }

    // rhd v0.1 has no /ui/state, and answers 404 for it.
    private async Task<PollResult> WithoutUiStateAsync(CancellationToken cancellationToken)
    {
        var health = await HealthAsync(cancellationToken).ConfigureAwait(false);
        return health is null
            ? new PollResult(ServiceStatus.Stopped, Detail: "GET /ui/state answered 404")
            : new PollResult(
                ServiceStatus.Incompatible,
                Version: health.Version,
                Detail: $"rhd {health.Version} has no /ui/state");
    }

    /// <returns><c>null</c> when there is no token to send.</returns>
    private async Task<HttpResponseMessage?> SendWithTokenAsync(
        HttpMethod method,
        string path,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        if (Token() is not { } token)
        {
            return null;
        }

        var response = await SendAsync(method, path, token, timeout, cancellationToken)
            .ConfigureAwait(false);
        if (response.StatusCode != HttpStatusCode.Unauthorized)
        {
            return response;
        }

        // rhd writes a new token when its token file was deleted, and this client may still
        // hold the old one.
        response.Dispose();
        _token = null;
        return Token() is { } fresh
            ? await SendAsync(method, path, fresh, timeout, cancellationToken)
                .ConfigureAwait(false)
            : null;
    }

    private async Task<HttpResponseMessage> SendAsync(
        HttpMethod method,
        string path,
        string token,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(method, path);
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        return await SendAsync(request, timeout, cancellationToken).ConfigureAwait(false);
    }

    // A timeout cancels the linked token only, so IsUnreachable tells it from the caller's own
    // cancellation.
    private async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        using var limit = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        limit.CancelAfter(timeout);
        return await http.SendAsync(request, limit.Token).ConfigureAwait(false);
    }

    private string? Token()
    {
        if (_token is not null)
        {
            return _token;
        }

        try
        {
            var token = File.ReadAllText(tokenFile).Trim();
            _token = token.Length == 0 ? null : token;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _token = null;
        }

        return _token;
    }
}
