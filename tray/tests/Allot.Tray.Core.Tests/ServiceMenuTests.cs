using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.TrayIcon;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class ServiceMenuTests
{
    private const string Footer =
        "Heavy commands run unchecked while the service is stopped. A Claude session starts it "
        + "again the next time it asks for resources.";

    [TestMethod]
    [DataRow(ServiceStatus.Running, "Stop service…", ServiceAction.Stop, true)]
    [DataRow(ServiceStatus.Incompatible, "Stop service…", ServiceAction.Stop, true)]
    [DataRow(ServiceStatus.Stopped, "Start service", ServiceAction.Start, true)]
    [DataRow(ServiceStatus.Starting, "Starting service…", ServiceAction.None, false)]
    public void ItemFollowsTheServiceStatus(
        ServiceStatus status,
        string text,
        ServiceAction action,
        bool enabled)
    {
        var item = ServiceMenu.ItemFor(status);

        Assert.AreEqual(text, item.Text);
        Assert.AreEqual(action, item.Action);
        Assert.AreEqual(enabled, item.Enabled);
    }

    [TestMethod]
    public void StopsWithoutAskingWhenNothingRunsOrWaits()
    {
        Assert.IsNull(ServiceMenu.StopConfirmation(Handoff.State(idle: true)));
        Assert.IsNull(ServiceMenu.StopConfirmation(null));
    }

    [TestMethod]
    public void AsksWhileOneJobRuns() =>
        Assert.AreEqual(
            $"1 job is running.\n\n{Footer}",
            ServiceMenu.StopConfirmation(Handoff.State(run: ["cargo"])));

    [TestMethod]
    public void CountsRunningJobsAndWaitingRequests() =>
        Assert.AreEqual(
            $"2 jobs are running and 1 request is waiting.\n\n{Footer}",
            ServiceMenu.StopConfirmation(Handoff.State(run: ["cargo", "py"], wait: ["npm"])));

    [TestMethod]
    public void AsksWhileOnlyRequestsWait() =>
        Assert.AreEqual(
            $"2 requests are waiting.\n\n{Footer}",
            ServiceMenu.StopConfirmation(Handoff.State(wait: ["npm", "ctest"])));
}
