using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

/// <summary>
/// HTTP client for the daemon routes the tray app uses. It never starts allotd by itself; that is
/// the user's Start service, through <see cref="ServiceLauncher"/>.
/// </summary>
/// <param name="http">Client with its base address set to the daemon; disposed with this
/// one. Each request sets its own timeout.</param>
/// <param name="tokenFile">The bearer token file, read on first use and after a 401.</param>
public sealed class DaemonClient(HttpClient http, string tokenFile)
    : IDisposable, IJobStopper, ISettingsClient
{
    private static readonly TimeSpan ConnectTimeout = TimeSpan.FromSeconds(1);
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(3);

    // allotd answers a kill after up to three sweeps over the tree, each waiting for the processes
    // it terminated.
    private static readonly TimeSpan StopTimeout = TimeSpan.FromSeconds(10);

    private string? _token;

    // Set once allotd moves to another port; until then the HttpClient's own base address holds.
    private volatile Uri? _base;

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
                    HttpMethod.Get, "ui/state", RequestTimeout, null, cancellationToken)
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
    /// <c>POST /shutdown</c> from Stop service: allotd writes the stop record, so no session
    /// starts it again, answers 202, then stops gracefully. Start service needs nothing of the
    /// kind, since every start of allotd clears the record. Only cancellation throws.
    /// </summary>
    /// <param name="record">Whether allotd writes the stop record; a restart sends no body, so
    /// nothing stays stopped between the stop and the start.</param>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <returns><c>null</c> when allotd accepted, else why not.</returns>
    public async Task<string?> ShutdownAsync(bool record, CancellationToken cancellationToken)
    {
        var body = record
            ? JsonSerializer.Serialize(
                new ShutdownRequest { By = StoppedBy.Panel },
                ContractJsonContext.Default.ShutdownRequest)
            : null;
        try
        {
            using var response = await SendWithTokenAsync(
                    HttpMethod.Post, "shutdown", RequestTimeout, body, cancellationToken)
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

    /// <summary>
    /// The port every request goes to now: the one the client started with, or the one
    /// <see cref="UsePort"/> moved it to.
    /// </summary>
    public ushort Port => (ushort)((_base ?? http.BaseAddress)?.Port ?? 0);

    /// <summary>
    /// Sends every later request to <paramref name="port"/>, after allotd restarted on a new one.
    /// </summary>
    public void UsePort(ushort port) =>
        _base = new Uri(string.Create(CultureInfo.InvariantCulture, $"http://127.0.0.1:{port}/"));

    /// <inheritdoc/>
    public Task<SettingsReply<SettingsState>> SettingsAsync(CancellationToken cancellationToken) =>
        SettingsRouteAsync(
            HttpMethod.Get,
            "settings",
            null,
            ContractJsonContext.Default.SettingsState,
            cancellationToken);

    /// <inheritdoc/>
    public Task<SettingsReply<SettingsState>> ChangeSettingsAsync(
        SettingsPatch patch,
        CancellationToken cancellationToken) =>
        SettingsRouteAsync(
            HttpMethod.Patch,
            "settings",
            JsonSerializer.Serialize(patch, ContractJsonContext.Default.SettingsPatch),
            ContractJsonContext.Default.SettingsState,
            cancellationToken);

    /// <inheritdoc/>
    public Task<SettingsReply<FitPreview>> PreviewSettingsAsync(
        IReadOnlyList<Change> changes,
        CancellationToken cancellationToken) =>
        SettingsRouteAsync(
            HttpMethod.Post,
            "settings/preview",
            JsonSerializer.Serialize(
                new PreviewRequest { Changes = changes },
                ContractJsonContext.Default.PreviewRequest),
            ContractJsonContext.Default.FitPreview,
            cancellationToken);

    /// <inheritdoc/>
    public Task<SettingsReply<CommandTest>> ClassifyAsync(
        ClassifyRequest request,
        CancellationToken cancellationToken) =>
        SettingsRouteAsync(
            HttpMethod.Post,
            "classify",
            JsonSerializer.Serialize(request, ContractJsonContext.Default.ClassifyRequest),
            ContractJsonContext.Default.CommandTest,
            cancellationToken);

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
    /// the job: allotd's text starts with the lease.
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
                    HttpMethod.Post, path, StopTimeout, null, cancellationToken)
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

    private static SettingsReply<T> Failed<T>(SettingsFailureKind kind, string message)
        where T : class =>
        new(new SettingsFailure(kind, message, [], null));

    // Settings routes refuse with a SettingsRefusal; a body that is not one, such as a 500's plain
    // text, keeps its text as the message.
    private static async Task<SettingsFailure> RefusalAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        var kind = response.StatusCode switch
        {
            HttpStatusCode.Conflict => SettingsFailureKind.Conflict,
            HttpStatusCode.UnprocessableEntity => SettingsFailureKind.Invalid,
            HttpStatusCode.Locked => SettingsFailureKind.Held,
            HttpStatusCode.ServiceUnavailable => SettingsFailureKind.Unavailable,
            _ => SettingsFailureKind.Failed,
        };
        var body = await response.Content.ReadAsStringAsync(cancellationToken)
            .ConfigureAwait(false);
        if (TryDecodeRefusal(body) is { } refusal)
        {
            return new SettingsFailure(kind, refusal.Message, refusal.Problems, refusal.Revision);
        }

        var text = body.Trim();
        var message = text.Length == 0
            ? $"the service answered {(int)response.StatusCode}"
            : text;
        return new SettingsFailure(kind, message, [], null);
    }

    private static SettingsRefusal? TryDecodeRefusal(string body)
    {
        try
        {
            return JsonSerializer.Deserialize(body, ContractJsonContext.Default.SettingsRefusal);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private async Task<SettingsReply<T>> SettingsRouteAsync<T>(
        HttpMethod method,
        string path,
        string? body,
        JsonTypeInfo<T> type,
        CancellationToken cancellationToken)
        where T : class
    {
        try
        {
            using var response = await SendWithTokenAsync(
                    method, path, RequestTimeout, body, cancellationToken)
                .ConfigureAwait(false);
            switch (response)
            {
                case null:
                    return Failed<T>(SettingsFailureKind.Token, $"no token in {tokenFile}");
                case { StatusCode: HttpStatusCode.OK }:
                    return await DecodeSettingsAsync(response, type, cancellationToken)
                        .ConfigureAwait(false);
                case { StatusCode: HttpStatusCode.Unauthorized }:
                    return Failed<T>(SettingsFailureKind.Token, "the service refused the token");
                default:
                    return new SettingsReply<T>(
                        await RefusalAsync(response, cancellationToken).ConfigureAwait(false));
            }
        }
        catch (Exception error) when (IsUnreachable(error, cancellationToken))
        {
            return Failed<T>(SettingsFailureKind.Unreachable, error.Message);
        }
    }

    private static async Task<SettingsReply<T>> DecodeSettingsAsync<T>(
        HttpResponseMessage response,
        JsonTypeInfo<T> type,
        CancellationToken cancellationToken)
        where T : class
    {
        var body = await response.Content.ReadAsStringAsync(cancellationToken)
            .ConfigureAwait(false);
        try
        {
            return JsonSerializer.Deserialize(body, type) is { } value
                ? new SettingsReply<T>(value)
                : Failed<T>(
                    SettingsFailureKind.Failed,
                    "the service's answer could not be read: it was empty");
        }
        catch (JsonException error)
        {
            return Failed<T>(
                SettingsFailureKind.Failed,
                $"the service's answer could not be read: {error.Message}");
        }
    }

    // allotd v0.1 has no /ui/state, and answers 404 for it.
    private async Task<PollResult> WithoutUiStateAsync(CancellationToken cancellationToken)
    {
        var health = await HealthAsync(cancellationToken).ConfigureAwait(false);
        return health is null
            ? new PollResult(ServiceStatus.Stopped, Detail: "GET /ui/state answered 404")
            : new PollResult(
                ServiceStatus.Incompatible,
                Version: health.Version,
                Detail: $"allotd {health.Version} has no /ui/state");
    }

    // A body is JSON, sent with its content type; each attempt builds its own content, so a request
    // refused with 401 can be sent again.
    /// <returns><c>null</c> when there is no token to send.</returns>
    private async Task<HttpResponseMessage?> SendWithTokenAsync(
        HttpMethod method,
        string path,
        TimeSpan timeout,
        string? body,
        CancellationToken cancellationToken)
    {
        if (Token() is not { } token)
        {
            return null;
        }

        var response = await SendAsync(method, path, token, timeout, body, cancellationToken)
            .ConfigureAwait(false);
        if (response.StatusCode != HttpStatusCode.Unauthorized)
        {
            return response;
        }

        // allotd writes a new token when its token file was deleted, and this client may still
        // hold the old one.
        response.Dispose();
        _token = null;
        return Token() is { } fresh
            ? await SendAsync(method, path, fresh, timeout, body, cancellationToken)
                .ConfigureAwait(false)
            : null;
    }

    private async Task<HttpResponseMessage> SendAsync(
        HttpMethod method,
        string path,
        string token,
        TimeSpan timeout,
        string? body,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(method, path);
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        if (body is not null)
        {
            request.Content = new StringContent(body, Encoding.UTF8, "application/json");
        }

        return await SendAsync(request, timeout, cancellationToken).ConfigureAwait(false);
    }

    // A timeout cancels the linked token only, so IsUnreachable tells it from the caller's own
    // cancellation.
    private async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        if (_base is { } moved && request.RequestUri is { IsAbsoluteUri: false } relative)
        {
            request.RequestUri = new Uri(moved, relative);
        }

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
