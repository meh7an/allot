using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.Daemon;

/// <summary>
/// HTTP client for the daemon routes the tray app reads. It never starts rhd by itself; that is
/// the user's Start service, through <see cref="ServiceLauncher"/>.
/// </summary>
/// <param name="http">Client with its base address set to the daemon; disposed with this
/// one.</param>
/// <param name="tokenFile">The bearer token file, read on first use and after a 401.</param>
public sealed class DaemonClient(HttpClient http, string tokenFile) : IDisposable
{
    private static readonly TimeSpan ConnectTimeout = TimeSpan.FromSeconds(1);
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(3);

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
            Timeout = RequestTimeout,
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
            using var response = await SendWithTokenAsync(HttpMethod.Get, "ui/state", cancellationToken)
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
            using var response = await http.GetAsync("health", cancellationToken)
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
                    HttpMethod.Post, "shutdown", cancellationToken)
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

    public void Dispose() => http.Dispose();

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
        CancellationToken cancellationToken)
    {
        if (Token() is not { } token)
        {
            return null;
        }

        var response = await SendAsync(method, path, token, cancellationToken)
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
            ? await SendAsync(method, path, fresh, cancellationToken).ConfigureAwait(false)
            : null;
    }

    private async Task<HttpResponseMessage> SendAsync(
        HttpMethod method,
        string path,
        string token,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(method, path);
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        return await http.SendAsync(request, cancellationToken).ConfigureAwait(false);
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
