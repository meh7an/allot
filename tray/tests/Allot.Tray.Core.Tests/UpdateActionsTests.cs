using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Tests.Support;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// A press of Update or Check now, from the press to the poll that shows what it did.
/// </summary>
[TestClass]
public sealed class UpdateActionsTests
{
    private readonly ManualTimeProvider _time = new();
    private readonly FakeUpdateClient _client = new();
    private readonly UpdateActions _actions;
    private int _polls;

    public UpdateActionsTests() =>
        _actions = new UpdateActions(_client, () => _polls++, _time, _ => { });

    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task AnAcceptedUpdateStaysPendingUntilThePollsSaySo()
    {
        _actions.Observe(Status(UpdateState.Available));

        await _actions.StartAsync(TestContext.CancellationToken);
        // A poll already under way when allotd answered.
        _actions.Observe(Status(UpdateState.Available));
        var stale = _actions.Press.Pending;
        _actions.Observe(Status(UpdateState.Installing));

        Assert.AreEqual(UpdateAction.Starting, stale);
        Assert.AreSame(UpdatePress.None, _actions.Press);
        Assert.AreEqual(1, _polls);
    }

    [TestMethod]
    public async Task ARunThatFailsAtOnceDoesNotLeaveUpdatingBehind()
    {
        _actions.Observe(Status(UpdateState.InstallFailed));
        await _actions.StartAsync(TestContext.CancellationToken);

        _time.Now += UpdateActions.StartHold;
        _actions.Observe(Status(UpdateState.InstallFailed));

        Assert.AreSame(UpdatePress.None, _actions.Press);
    }

    [TestMethod]
    public async Task ARefusalIsKeptUntilTheStateMovesOn()
    {
        _client.Problem = "an update is already running";
        _actions.Observe(Status(UpdateState.Available));

        await _actions.StartAsync(TestContext.CancellationToken);
        _actions.Observe(Status(UpdateState.Available));
        var kept = _actions.Press.Problem;
        _actions.Observe(Status(UpdateState.Installing));

        Assert.AreEqual("an update is already running", kept);
        Assert.IsNull(_actions.Press.Problem);
    }

    [TestMethod]
    public async Task AnAnsweredCheckIsOver()
    {
        await _actions.CheckAsync(TestContext.CancellationToken);

        Assert.AreSame(UpdatePress.None, _actions.Press);
        Assert.AreEqual("check", string.Join(" ", _client.Calls));
        Assert.AreEqual(1, _polls);
    }

    [TestMethod]
    public async Task ASecondPressWaitsForTheFirst()
    {
        var answer = new TaskCompletionSource<string?>();
        _client.Answer = answer.Task;
        var shown = new List<UpdateAction>();
        _actions.Changed += () => shown.Add(_actions.Press.Pending);

        var first = _actions.StartAsync(TestContext.CancellationToken);
        await _actions.StartAsync(TestContext.CancellationToken);
        await _actions.CheckAsync(TestContext.CancellationToken);
        answer.SetResult(null);
        await first;

        Assert.AreEqual("start", string.Join(" ", _client.Calls));
        CollectionAssert.AreEqual(new[] { UpdateAction.Starting }, shown);
    }

    private static UpdateStatus Status(UpdateState state) =>
        new()
        {
            Installed = "0.6.1",
            State = state,
            Latest = "0.7.0",
            Notes = [],
        };

    private sealed class FakeUpdateClient : IUpdateClient
    {
        public List<string> Calls { get; } = [];

        public string? Problem { get; set; }

        public Task<string?>? Answer { get; set; }

        public Task<string?> CheckForUpdateAsync(CancellationToken cancellationToken) =>
            Respond("check");

        public Task<string?> StartUpdateAsync(CancellationToken cancellationToken) =>
            Respond("start");

        public Task<string?> UpdateAnnouncedAsync(
            string version,
            CancellationToken cancellationToken) => Respond($"announced {version}");

        private Task<string?> Respond(string call)
        {
            Calls.Add(call);
            return Answer ?? Task.FromResult(Problem);
        }
    }
}
