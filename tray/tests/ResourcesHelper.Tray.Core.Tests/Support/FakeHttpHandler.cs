using System.Collections.Concurrent;
using System.Net;
using System.Text;

namespace ResourcesHelper.Tray.Core.Tests.Support;

/// <summary>
/// Answers every request with <c>respond</c> and records what was asked. Throwing from
/// <c>respond</c> fails the request as a refused connection would.
/// </summary>
internal sealed class FakeHttpHandler(Func<HttpRequestMessage, HttpResponseMessage> respond)
    : HttpMessageHandler
{
    public ConcurrentQueue<SeenRequest> Seen { get; } = new();

    public static HttpResponseMessage Json(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) =>
        new(status) { Content = new StringContent(json, Encoding.UTF8, "application/json") };

    public static HttpResponseMessage Status(HttpStatusCode status) => new(status);

    public static HttpRequestException Refused() =>
        new("No connection could be made because the target machine actively refused it.");

    protected override Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        Seen.Enqueue(new SeenRequest(
            request.RequestUri?.AbsolutePath ?? "",
            request.Headers.Authorization?.Parameter));
        try
        {
            return Task.FromResult(respond(request));
        }
        catch (HttpRequestException error)
        {
            return Task.FromException<HttpResponseMessage>(error);
        }
    }
}

internal sealed record SeenRequest(string Path, string? Token);
