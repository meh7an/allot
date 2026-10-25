using System.Net;

using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// <c>POST /leases/{id}/kill</c> and <c>/remove</c> through <see cref="DaemonClient"/>: allotd's
/// answers become the Stopped note or the reason a card shows in red.
/// </summary>
[TestClass]
public sealed class DaemonStopTests
{
    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task KillPostsToTheLeaseWithTheToken()
    {
        using var directory = new TempDirectory();
        var methods = new List<HttpMethod>();
        var handler = new FakeHttpHandler(request =>
        {
            methods.Add(request.Method);
            return FakeHttpHandler.Json(Fixtures.Read("kill-response-untouched.json"));
        });
        using var client = DaemonClientTests.Client(directory, handler, "secret");

        _ = await client.KillAsync(39, TestContext.CancellationToken);

        Assert.AreEqual(new SeenRequest("/leases/39/kill", "secret"), handler.Seen.Single());
        Assert.AreEqual(HttpMethod.Post, methods.Single());
    }

    [TestMethod]
    public async Task KillOfATreeWithAFailureNotesTheSurvivor()
    {
        var result = await KillAnswering(
            FakeHttpHandler.Json(Fixtures.Read("kill-response-tree.json")));

        Assert.AreEqual(new StopResult(true, "1 process still running", 1_792_824_700_000), result);
    }

    [TestMethod]
    public async Task KillWithNothingBoundNotesThatNothingRan()
    {
        var result = await KillAnswering(
            FakeHttpHandler.Json(Fixtures.Read("kill-response-untouched.json")));

        Assert.IsTrue(result.Ended);
        Assert.AreEqual("nothing was running", result.Text);
    }

    [TestMethod]
    public async Task RemoveEndsWithoutANote()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(
            _ => FakeHttpHandler.Json(Fixtures.Read("remove-response.json")));
        using var client = DaemonClientTests.Client(directory, handler, "secret");

        var result = await client.RemoveAsync(41, TestContext.CancellationToken);

        Assert.AreEqual(new StopResult(true, "", 1_792_824_720_000), result);
        Assert.AreEqual("/leases/41/remove", handler.Seen.Single().Path);
    }

    [TestMethod]
    [DataRow(
        "lease 39 was not killed: the bound process 18244 (cargo.exe) is protected",
        "the bound process 18244 (cargo.exe) is protected")]
    [DataRow(
        "lease 39 was not killed: the bound process 18244 (cargo.exe) started before the "
            + "command was reported",
        "the bound process 18244 (cargo.exe) started before the command was reported")]
    [DataRow("lease 39 has already ended (released)", "the job has already ended")]
    [DataRow("lease 39 is queued; remove it instead", "lease 39 is queued; remove it instead")]
    [DataRow("", "the service refused")]
    public async Task ConflictShowsRhdsReasonWithoutTheLease(string body, string reason)
    {
        var result = await KillAnswering(new HttpResponseMessage(HttpStatusCode.Conflict)
        {
            Content = new StringContent(body),
        });

        Assert.AreEqual(StopResult.Refused(reason), result);
    }

    [TestMethod]
    [DataRow(HttpStatusCode.NotFound, "the job no longer exists")]
    [DataRow(HttpStatusCode.ServiceUnavailable, "the service has just started; try again")]
    [DataRow(HttpStatusCode.InternalServerError, "the service answered 500")]
    public async Task OtherStatusesSayWhatHappened(HttpStatusCode status, string reason)
    {
        var result = await KillAnswering(FakeHttpHandler.Status(status));

        Assert.AreEqual(StopResult.Refused(reason), result);
    }

    [TestMethod]
    public async Task UnreachableServiceIsARefusal()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => throw FakeHttpHandler.Refused());
        using var client = DaemonClientTests.Client(directory, handler, "secret");

        var result = await client.RemoveAsync(41, TestContext.CancellationToken);

        Assert.AreEqual(StopResult.Refused("the service is not running"), result);
    }

    [TestMethod]
    public async Task UnreadableAnswerIsARefusal()
    {
        var result = await KillAnswering(FakeHttpHandler.Json("""{"lease":42}"""));

        Assert.AreEqual(StopResult.Refused("the service's answer could not be read"), result);
    }

    [TestMethod]
    public async Task WithoutATokenNothingIsSent()
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => FakeHttpHandler.Status(HttpStatusCode.OK));
        using var client = DaemonClientTests.Client(directory, handler, token: null);

        var result = await client.KillAsync(39, TestContext.CancellationToken);

        Assert.IsFalse(result.Ended);
        Assert.IsEmpty(handler.Seen);
    }

    [TestMethod]
    public void SeveralFailuresAreCounted()
    {
        var response = new Contract.KillResponse
        {
            Failed =
            [
                new Contract.UnkilledProcess { Name = "a.exe", Pid = 1, Reason = "denied" },
                new Contract.UnkilledProcess { Name = "b.exe", Pid = 2, Reason = "denied" },
            ],
            Lease = Samples.Handoff.State(run: ["cargo"]).Active[0].Lease,
            Spared = [],
            Terminated = [],
        };

        Assert.AreEqual("2 processes still running", StopResult.Killed(response).Text);
    }

    private async Task<StopResult> KillAnswering(HttpResponseMessage answer)
    {
        using var directory = new TempDirectory();
        var handler = new FakeHttpHandler(_ => answer);
        using var client = DaemonClientTests.Client(directory, handler, "secret");
        return await client.KillAsync(39, TestContext.CancellationToken);
    }
}
