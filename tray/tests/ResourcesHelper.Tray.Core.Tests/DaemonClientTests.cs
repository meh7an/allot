using System.Net;

using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Tests.Support;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class DaemonClientTests
{
    private const string HealthJson =
        """{"pid":4242,"uptime_secs":60,"version":"0.1.0","config_error":null}""";

    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task RunningDaemonServesStateWithTheToken()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(
            _ => FakeHttpHandler.Json(Fixtures.Read("ui-state-busy.json")));
        using var client = Client(directory, handler, "secret\r\n");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Running, result.Status);
        Assert.IsNotNull(result.State);
        Assert.AreEqual(result.State.Service.Version, result.Version);
        Assert.AreEqual(new SeenRequest("/ui/state", "secret"), handler.Seen.Single());
    }

    [TestMethod]
    public async Task RefusedTokenIsReadAgainOnce()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(request =>
            request.Headers.Authorization?.Parameter == "new"
                ? FakeHttpHandler.Json(Fixtures.Read("ui-state-idle.json"))
                : FakeHttpHandler.Status(HttpStatusCode.Unauthorized));
        using var client = Client(directory, handler, "old");
        _ = await client.PollAsync(TestContext.CancellationToken);
        File.WriteAllText(directory.File("token"), "new");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Running, result.Status);
        Assert.AreEqual(
            "old | old | old | new",
            string.Join(" | ", handler.Seen.Select(seen => seen.Token)));
    }

    [TestMethod]
    public async Task TokenStillRefusedAfterRereadIsIncompatible()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => FakeHttpHandler.Status(HttpStatusCode.Unauthorized));
        using var client = Client(directory, handler, "old");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Incompatible, result.Status);
        Assert.AreEqual(2, handler.Seen.Count);
    }

    [TestMethod]
    public async Task RefusedConnectionIsStopped()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => throw FakeHttpHandler.Refused());
        using var client = Client(directory, handler, "secret");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Stopped, result.Status);
        Assert.IsNull(result.State);
    }

    [TestMethod]
    public async Task MissingTokenIsStoppedWithoutARequest()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => FakeHttpHandler.Status(HttpStatusCode.OK));
        using var client = Client(directory, handler, token: null);

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Stopped, result.Status);
        Assert.IsEmpty(handler.Seen);
    }

    [TestMethod]
    public async Task OldDaemonWithoutUiStateIsIncompatible()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(request =>
            request.RequestUri!.AbsolutePath == "/health"
                ? FakeHttpHandler.Json(HealthJson)
                : FakeHttpHandler.Status(HttpStatusCode.NotFound));
        using var client = Client(directory, handler, "secret");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Incompatible, result.Status);
        Assert.AreEqual("0.1.0", result.Version);
        Assert.IsNull(handler.Seen.Last().Token, "/health is sent without the token");
    }

    [TestMethod]
    public async Task NoFirstSampleYetIsStarting()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(
            _ => FakeHttpHandler.Status(HttpStatusCode.ServiceUnavailable));
        using var client = Client(directory, handler, "secret");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Starting, result.Status);
    }

    [TestMethod]
    public async Task StateTheTypesRejectIsIncompatible()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => FakeHttpHandler.Json("""{"active":[]}"""));
        using var client = Client(directory, handler, "secret");

        var result = await client.PollAsync(TestContext.CancellationToken);

        Assert.AreEqual(ServiceStatus.Incompatible, result.Status);
        Assert.IsNotNull(result.Detail);
    }

    [TestMethod]
    public async Task HealthDecodesWithoutAToken()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => FakeHttpHandler.Json(HealthJson));
        using var client = Client(directory, handler, token: null);

        var health = await client.HealthAsync(TestContext.CancellationToken);

        Assert.AreEqual("0.1.0", health?.Version);
    }

    internal static DaemonClient Client(
        TempDirectory directory,
        FakeHttpHandler handler,
        string? token)
    {
        if (token is not null)
        {
            File.WriteAllText(directory.File("token"), token);
        }

        var http = new HttpClient(handler) { BaseAddress = new Uri("http://127.0.0.1:47823/") };
        return new DaemonClient(http, directory.File("token"));
    }
}
