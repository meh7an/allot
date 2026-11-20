using Allot.Tray.Core.Flyout;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class TaskbarEdgeTests
{
    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 1.0)]
    [DataRow(TaskbarEdge.Top, 1.0)]
    [DataRow(TaskbarEdge.Left, 1.0)]
    [DataRow(TaskbarEdge.Right, 1.0)]
    [DataRow(TaskbarEdge.Bottom, 1.5)]
    [DataRow(TaskbarEdge.Top, 1.5)]
    [DataRow(TaskbarEdge.Left, 1.5)]
    [DataRow(TaskbarEdge.Right, 1.5)]
    public void ReadsTheEdgeFromTheTaskbarRectangle(TaskbarEdge edge, double scale) =>
        Assert.AreEqual(
            edge,
            TaskbarEdges.FromBounds(Taskbars.Bounds(edge, scale), Taskbars.Monitor(scale)));

    [TestMethod]
    public void ReadsALeftTaskbarOnAMonitorLeftOfThePrimary()
    {
        var monitor = new PixelRect(-1920, 200, 1920, 1080);

        var edge = TaskbarEdges.FromBounds(new PixelRect(-1920, 200, 48, 1080), monitor);

        Assert.AreEqual(TaskbarEdge.Left, edge);
    }

    [TestMethod]
    public void ReadsAnAutoHiddenTaskbarThatHangsOffTheMonitor()
    {
        // Hidden, all but a 2 px strip of a bottom taskbar is below the monitor.
        var hidden = new PixelRect(0, 1078, 1920, 48);

        Assert.AreEqual(
            TaskbarEdge.Bottom, TaskbarEdges.FromBounds(hidden, Taskbars.Monitor(1.0)));
    }

    [TestMethod]
    public void AnEmptyTaskbarRectangleTellsNothing() =>
        Assert.IsNull(TaskbarEdges.FromBounds(default, Taskbars.Monitor(1.0)));

    [TestMethod]
    [DataRow(TaskbarEdge.Bottom)]
    [DataRow(TaskbarEdge.Top)]
    [DataRow(TaskbarEdge.Left)]
    [DataRow(TaskbarEdge.Right)]
    public void ReadsTheEdgeFromTheWorkArea(TaskbarEdge edge) =>
        Assert.AreEqual(
            edge,
            TaskbarEdges.FromWorkArea(Taskbars.WorkArea(edge, 1.0), Taskbars.Monitor(1.0)));

    [TestMethod]
    public void AFullWorkAreaTellsNothing() =>
        Assert.IsNull(TaskbarEdges.FromWorkArea(Taskbars.Monitor(1.0), Taskbars.Monitor(1.0)));

    [TestMethod]
    public void AWorkAreaShortOnTwoSidesTellsNothing() =>
        Assert.IsNull(
            TaskbarEdges.FromWorkArea(new PixelRect(48, 0, 1872, 1032), Taskbars.Monitor(1.0)));

    [TestMethod]
    public void TheTaskbarRectangleWinsOverTheWorkArea()
    {
        // Another appbar holds the bottom of the work area; the taskbar is on the left.
        var workArea = new PixelRect(0, 0, 1920, 1000);

        var edge = TaskbarEdges.Resolve(
            Taskbars.Bounds(TaskbarEdge.Left, 1.0), workArea, Taskbars.Monitor(1.0));

        Assert.AreEqual(TaskbarEdge.Left, edge);
    }

    [TestMethod]
    public void FallsBackToTheWorkAreaWithoutATaskbarRectangle() =>
        Assert.AreEqual(
            TaskbarEdge.Right,
            TaskbarEdges.Resolve(
                null, Taskbars.WorkArea(TaskbarEdge.Right, 1.0), Taskbars.Monitor(1.0)));

    [TestMethod]
    public void FallsBackToTheBottomWhenNothingTells() =>
        Assert.AreEqual(
            TaskbarEdge.Bottom,
            TaskbarEdges.Resolve(null, Taskbars.Monitor(1.0), Taskbars.Monitor(1.0)));
}
