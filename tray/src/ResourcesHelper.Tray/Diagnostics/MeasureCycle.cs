using ResourcesHelper.Tray.Flyout;

namespace ResourcesHelper.Tray.Diagnostics;

/// <summary>
/// The fixed run behind <c>RH_TRAY_MEASURE=cycle</c>, so before and after numbers come from the
/// same sequence: three minutes closed, ten opens and closes, six Resources toggles, then three
/// minutes open with Stay open, each phase ending with a usage line; then the app quits. Opens go
/// through the tray click's path without its foreground right, so the window may stay inactive.
/// </summary>
internal static class MeasureCycle
{
    private static readonly TimeSpan Settle = TimeSpan.FromSeconds(15);
    private static readonly TimeSpan Minute = TimeSpan.FromSeconds(60);
    private static readonly TimeSpan Shown = TimeSpan.FromSeconds(2);
    private static readonly TimeSpan Between = TimeSpan.FromMilliseconds(1500);
    private static readonly TimeSpan StressFor = TimeSpan.FromSeconds(150);
    private static readonly TimeSpan StartupFor = TimeSpan.FromSeconds(4);
    private const int Minutes = 3;
    private const int SoakMinutes = 10;
    private const int Opens = 10;
    // Even, so the card ends as it started.
    private const int ResourcesToggles = 6;

    /// <param name="flyout">The flyout window.</param>
    /// <param name="toggleResources">Expands or collapses the Resources card, as its header
    /// does.</param>
    /// <param name="quit">Ends the app once the run is done, so nothing has to kill it.</param>
    /// <param name="cancellationToken">Quit.</param>
    public static async Task RunAsync(
        FlyoutWindow flyout,
        Action toggleResources,
        Action quit,
        CancellationToken cancellationToken)
    {
        try
        {
            // Resumes on the UI thread after every wait.
            await Task.Delay(Settle, cancellationToken).ConfigureAwait(true);
            Measure.Write("cycle: startup done");
            Measure.Lap();

            Measure.Write($"cycle: closed for {Minutes} min");
            await IdleAsync(cancellationToken).ConfigureAwait(true);

            Measure.Write($"cycle: {Opens} opens and closes");
            for (var i = 0; i < Opens; i++)
            {
                flyout.Toggle();
                await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
                flyout.Toggle();
                await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
            }

            Measure.Lap();
            Measure.Write($"cycle: {ResourcesToggles} Resources toggles");
            flyout.Toggle();
            await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
            for (var i = 0; i < ResourcesToggles; i++)
            {
                toggleResources();
                await Task.Delay(Between, cancellationToken).ConfigureAwait(true);
            }

            Measure.Lap();
            Measure.Write($"cycle: open with Stay open for {Minutes} min");
            flyout.SetStayOpen(true);
            await IdleAsync(cancellationToken).ConfigureAwait(true);

            flyout.Toggle();
            await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
            Measure.Write("cycle: done");
            quit();
        }
        catch (OperationCanceledException)
        {
            // Quit.
        }
    }

    /// <summary>
    /// <c>RH_TRAY_MEASURE=stress</c>: the flyout opens with Stay open and Pin after the settle
    /// time and stays open for <see cref="StressFor"/> while a driver floods rhd, then the app
    /// quits.
    /// </summary>
    public static async Task StressAsync(
        FlyoutWindow flyout,
        Action quit,
        CancellationToken cancellationToken)
    {
        try
        {
            await Task.Delay(Settle, cancellationToken).ConfigureAwait(true);
            Measure.Lap();
            Measure.Write("stress: open with Stay open and Pin");
            flyout.Toggle();
            flyout.SetStayOpen(true);

            // Without the click's foreground right the window may open behind the active one;
            // pinned, it is topmost and can be watched.
            flyout.SetPinned(true);
            await Task.Delay(StressFor, cancellationToken).ConfigureAwait(true);
            Measure.Lap();
            flyout.Toggle();
            await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
            Measure.Write("stress: done");
            quit();
        }
        catch (OperationCanceledException)
        {
            // Quit.
        }
    }

    /// <summary>
    /// <c>RH_TRAY_MEASURE=soak</c>: open with Stay open and Pin for <see cref="SoakMinutes"/>
    /// minutes with a usage line each minute, to show memory does not drift, then quits.
    /// </summary>
    public static async Task SoakAsync(
        FlyoutWindow flyout,
        Action quit,
        CancellationToken cancellationToken)
    {
        try
        {
            await Task.Delay(Settle, cancellationToken).ConfigureAwait(true);
            Measure.Lap();
            Measure.Write($"soak: open for {SoakMinutes} min");
            flyout.Toggle();
            flyout.SetStayOpen(true);
            flyout.SetPinned(true);
            for (var i = 0; i < SoakMinutes; i++)
            {
                await Task.Delay(Minute, cancellationToken).ConfigureAwait(true);
                Measure.Lap();
            }

            flyout.Toggle();
            await Task.Delay(Shown, cancellationToken).ConfigureAwait(true);
            Measure.Write("soak: done");
            quit();
        }
        catch (OperationCanceledException)
        {
            // Quit.
        }
    }

    /// <summary>
    /// <c>RH_TRAY_MEASURE=startup</c>: quits a few seconds after launch, once the startup marks
    /// are written, so launches can be timed one after another.
    /// </summary>
    public static async Task StartupAsync(Action quit, CancellationToken cancellationToken)
    {
        try
        {
            await Task.Delay(StartupFor, cancellationToken).ConfigureAwait(true);
            Measure.Write("startup: done");
            quit();
        }
        catch (OperationCanceledException)
        {
            // Quit.
        }
    }

    private static async Task IdleAsync(CancellationToken cancellationToken)
    {
        for (var i = 0; i < Minutes; i++)
        {
            await Task.Delay(Minute, cancellationToken).ConfigureAwait(true);
            Measure.Lap();
        }
    }
}
