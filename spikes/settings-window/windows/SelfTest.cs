using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

using Windows.Graphics;

namespace SettingsSpike;

/// <summary>
/// <c>allot-settings-spike.exe --selftest</c>: the checks that need no hands. Lists the displays,
/// opens and closes the window 20 times under each close model and counts the windows left alive,
/// round-trips the placement, walks every page and backdrop, then quits with exit code 0 when
/// everything held. Results go to the log; "RESULT" lines are the summary.
/// </summary>
internal static class SelfTest
{
    public static bool Requested { get; } = Environment.GetCommandLineArgs().Contains("--selftest");

    public static async Task<bool> RunAsync(SettingsHost host)
    {
        var passed = true;
        void Result(bool ok, string what)
        {
            passed &= ok;
            SpikeLog.Write($"RESULT {(ok ? "pass" : "FAIL")}: {what}");
        }

        // Never steal focus from whatever Mehran is doing.
        host.ForceForeground = false;
        PlacementStore.Forget();
        LogDisplays();
        SpikeLog.Write($"baseline private {SettingsHost.PrivateMb()} MB");

        try
        {
            // Which part keeps a closed window alive: each probe opens and closes 10 windows.
            (string Name, Func<Window> Make)[] probes =
            [
                ("plain Window", () => new Window()),
                ("Window with a Grid", () => new Window { Content = new Grid() }),
                ("Window with Mica", () => new Window { SystemBackdrop = new MicaBackdrop() }),
                ("Window with x:Bind in its XAML", () => new BoundWindow()),
            ];
            foreach (var (name, make) in probes)
            {
                var alive = await ProbeAsync(make, 10).ConfigureAwait(true);
                SpikeLog.Write(
                    $"RESULT info: leak probe, {name}: {alive} of 10 closed windows alive, "
                    + $"private {SettingsHost.PrivateMb()} MB");
            }

            host.CloseModel = CloseModel.Destroy;
            var destroyed = await host.CycleAsync(20).ConfigureAwait(true);
            Result(destroyed.Alive == 0,
                $"destroy model: {destroyed.Alive} of {destroyed.Created} closed windows still alive, "
                + $"private {destroyed.PrivateMb} MB");

            // Does private memory level off (caches) or keep rising (a native leak per window)?
            for (var round = 2; round <= 4; round++)
            {
                var more = await host.CycleAsync(20).ConfigureAwait(true);
                SpikeLog.Write(
                    $"RESULT info: destroy model after {round * 20} opens: {more.Alive} alive, "
                    + $"private {more.PrivateMb} MB");
            }

            host.CloseModel = CloseModel.Hide;
            var hidden = await host.CycleAsync(20).ConfigureAwait(true);
            var reused = hidden.Created - destroyed.Created - 60;
            Result(reused == 1 && hidden.OneOpen,
                $"hide model: {reused} window built for 20 opens, private {hidden.PrivateMb} MB");
            for (var round = 2; round <= 3; round++)
            {
                var more = await host.CycleAsync(20).ConfigureAwait(true);
                SpikeLog.Write($"RESULT info: hide model after {round * 20} opens: private {more.PrivateMb} MB");
            }

            host.Shutdown();
            var afterHide = await host.CountAliveAsync().ConfigureAwait(true);
            Result(afterHide.Alive == 0, $"after closing the reused window: {afterHide.Alive} alive");

            host.CloseModel = CloseModel.Destroy;
            Result(await PlacementRoundTrip(host).ConfigureAwait(true), "placement restored after close and reopen");

            var window = host.Open("selftest pages");
            await window.ContentLoaded.ConfigureAwait(true);
            for (var i = 0; i < window.PageCount; i++)
            {
                window.SelectPage(i);
                await Task.Delay(150).ConfigureAwait(true);
                Result(window.PageType is not null, $"page {window.PageTag} shows {window.PageType?.Name}");
            }

            for (var i = 0; i < window.BackdropCount; i++)
            {
                window.SelectBackdrop(i);
                await Task.Delay(150).ConfigureAwait(true);
                SpikeLog.Write($"backdrop {i}: {window.BackdropName}");
            }

            Result(true, "every backdrop set without an exception");
            window.Close();
        }
        catch (Exception error)
        {
            Result(false, $"threw {error}");
        }

        return passed;
    }

    private static async Task<int> ProbeAsync(Func<Window> make, int times)
    {
        var created = new List<WeakReference<Window>>();
        for (var i = 0; i < times; i++)
        {
            await ProbeOnceAsync(make, created).ConfigureAwait(true);
        }

        await SettingsHost.CollectAsync().ConfigureAwait(true);
        return created.Count(reference => reference.TryGetTarget(out _));
    }

    private static async Task ProbeOnceAsync(Func<Window> make, List<WeakReference<Window>> created)
    {
        var window = make();
        created.Add(new WeakReference<Window>(window));
        window.Activate();
        await Task.Delay(150).ConfigureAwait(true);
        _ = NativeMethods.PostMessage(
            Win32Interop.GetWindowFromWindowId(window.AppWindow.Id), NativeMethods.WmClose, 0, 0);
        await Task.Delay(120).ConfigureAwait(true);
    }

    private static async Task<bool> PlacementRoundTrip(SettingsHost host)
    {
        var first = host.Open("selftest placement");
        await first.ContentLoaded.ConfigureAwait(true);
        var display = DisplayArea.GetFromWindowId(first.AppWindow.Id, DisplayAreaFallback.Nearest);
        var scale = SettingsHost.MonitorScale(display);
        var work = display.WorkArea;
        var target = new RectInt32(work.X + 120, work.Y + 90, (int)(700 * scale), (int)(520 * scale));
        first.AppWindow.MoveAndResize(target);
        await Task.Delay(200).ConfigureAwait(true);
        _ = NativeMethods.PostMessage(first.Hwnd, NativeMethods.WmClose, 0, 0);
        await Task.Delay(300).ConfigureAwait(true);

        var second = host.Open("selftest placement reopen");
        await second.ContentLoaded.ConfigureAwait(true);
        var position = second.AppWindow.Position;
        var size = second.AppWindow.Size;
        SpikeLog.Write(
            $"placement: set {target.Width}x{target.Height} at {target.X},{target.Y}, "
            + $"reopened {size.Width}x{size.Height} at {position.X},{position.Y}");
        var same = Math.Abs(position.X - target.X) <= 1 && Math.Abs(position.Y - target.Y) <= 1
            && Math.Abs(size.Width - target.Width) <= 2 && Math.Abs(size.Height - target.Height) <= 2;
        second.Close();
        await Task.Delay(200).ConfigureAwait(true);
        return same;
    }

    private static void LogDisplays()
    {
        var displays = DisplayArea.FindAll();
        for (var i = 0; i < displays.Count; i++)
        {
            var display = displays[i];
            var bounds = display.OuterBounds;
            SpikeLog.Write(
                $"display {display.DisplayId.Value}: {bounds.Width}x{bounds.Height} at {bounds.X},{bounds.Y}, "
                + $"scale {SettingsHost.MonitorScale(display):0.##}{(display.IsPrimary ? ", primary" : "")}");
        }
    }
}
