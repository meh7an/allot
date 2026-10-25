using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// What the 2 px edge bar along a card's bottom shows.
/// </summary>
public enum EdgeBarTone
{
    /// <summary>
    /// Green: elapsed against the estimate.
    /// </summary>
    Running,

    /// <summary>
    /// Amber and full: past the estimate.
    /// </summary>
    Over,

    /// <summary>
    /// Amber: waited against the expected wait.
    /// </summary>
    Waiting,

    /// <summary>
    /// Amber dashes across the whole card: the wait has no estimate.
    /// </summary>
    WaitingDashed,

    /// <summary>
    /// Gray and still: a stopped job, or last known state while the service is down.
    /// </summary>
    Frozen,

    /// <summary>
    /// No bar and no track: a removed request.
    /// </summary>
    None,
}

public enum CardAction
{
    /// <summary>
    /// A 32 px spacer keeps the layout.
    /// </summary>
    None,

    /// <summary>
    /// The Kill icon button of a running job.
    /// </summary>
    Kill,

    /// <summary>
    /// The red Kill? button that confirms a kill.
    /// </summary>
    Confirm,

    /// <summary>
    /// The Remove text button of a queued request.
    /// </summary>
    Remove,
}

/// <summary>
/// The job row of a card at one moment (handoff section 4).
/// </summary>
/// <param name="Tile">Two-letter initials of the process, or <c>#N</c> for a queued job.</param>
/// <param name="IsQueued">The tile is the dashed queue position.</param>
/// <param name="ExePath">The process executable, whose icon replaces the initials when it can be
/// read; <c>null</c> for queued jobs and unknown paths.</param>
/// <param name="Chip">The class chip label.</param>
/// <param name="ChipTooltip">The chip's tooltip.</param>
/// <param name="IsBenchmark">The chip is the solid purple Benchmark chip.</param>
/// <param name="Strong">The semibold start of the detail line, <c>Stopped</c> or
/// <c>Removed</c>; empty otherwise.</param>
/// <param name="Detail">The detail line's left part, after <paramref name="Strong"/>.</param>
/// <param name="DetailTooltip">The detail's tooltip, or empty for none.</param>
/// <param name="DetailIsDanger">The detail reads in red: the Kill? confirm, or a refusal.</param>
/// <param name="Time">The detail line's right part.</param>
/// <param name="TimeIsWarning">The time reads in amber: past the estimate, or waiting.</param>
/// <param name="BarFraction">How much of the edge bar is filled, from 0 to 1.</param>
/// <param name="BarTone">The edge bar's colour and pattern.</param>
/// <param name="Action">The control at the row's right.</param>
/// <param name="ActionName">The action's accessible name, for example
/// <c>Kill cargo.exe</c>.</param>
/// <param name="IsEnded">The job has ended (stopped, removed, finished, ended by allotd): the tile,
/// command and chip show at 50%.</param>
/// <param name="IsStruck">The command is struck through: stopped or removed, not finished.</param>
/// <param name="Icon">The detail line's leading icon; with one, the line wraps instead of
/// truncating.</param>
/// <param name="Steps">A benchmark's Drain / Quiet / Run stepper, or <c>null</c> for none.</param>
/// <param name="Announcement">What Narrator says when a stop ends or is refused, for example
/// <c>Stopped cargo.exe · Claude was notified</c>; empty otherwise.</param>
public sealed record JobCard(
    string Tile,
    bool IsQueued,
    string? ExePath,
    string Chip,
    string ChipTooltip,
    bool IsBenchmark,
    string Strong,
    string Detail,
    string DetailTooltip,
    bool DetailIsDanger,
    string Time,
    bool TimeIsWarning,
    double BarFraction,
    EdgeBarTone BarTone,
    CardAction Action,
    string ActionName,
    bool IsEnded,
    DetailIcon Icon = DetailIcon.None,
    BenchmarkSteps? Steps = null,
    string Announcement = "",
    bool IsStruck = false)
{
    // A queued job this close to its start reads "starts next".
    private const ulong StartsNextSecs = 60;

    /// <param name="job">The running job.</param>
    /// <param name="nowMs">The moment to show, in the daemon's clock.</param>
    /// <param name="lastKnown">The job comes from the last state allotd served before it went
    /// away.</param>
    /// <param name="stop">A Kill under way or done in this flyout, or <c>null</c>; ignored for
    /// last known state.</param>
    public static JobCard ForActive(
        ActiveJob job,
        long nowMs,
        bool lastKnown,
        StopView? stop = null)
    {
        var card = ActiveCard(job, nowMs, lastKnown, stop);
        if (!card.IsBenchmark)
        {
            return card;
        }

        var lease = job.Lease;
        var elapsed = Seconds(nowMs - (lease.GrantedAtMs ?? lease.RequestedAtMs));
        var status = BenchmarkStatus.ForActive(job, elapsed);
        return Benchmark(card, status, lastKnown, stop is { Kind: StopKind.Kill } ? stop : null);
    }

    /// <summary>
    /// A benchmark that finished on its own: the green check, the run time and its background
    /// noise, every step complete, and no action.
    /// </summary>
    /// <param name="run">The finished run.</param>
    /// <param name="noise">Its background load.</param>
    public static JobCard ForFinished(RecentRun run, NoiseReport noise)
    {
        var status = BenchmarkStatus.ForFinished(run, noise);
        return new JobCard(
            Format.Initials(FirstWord(run.Command)),
            false,
            null,
            "Benchmark",
            "Benchmark",
            true,
            "",
            status.Text,
            "",
            false,
            "",
            false,
            0,
            EdgeBarTone.None,
            CardAction.None,
            "",
            false,
            status.Icon,
            status.Steps);
    }

    private static JobCard ActiveCard(
        ActiveJob job,
        long nowMs,
        bool lastKnown,
        StopView? stop)
    {
        var lease = job.Lease;
        var process = job.Process;
        var started = lease.GrantedAtMs ?? lease.RequestedAtMs;
        var elapsed = Seconds(nowMs - started);
        var estimate = (long)lease.Profile.EstSecs;
        var over = estimate > 0 && elapsed > estimate;
        var progress = estimate > 0 ? Fraction(elapsed, estimate) : 0;
        var time = estimate > 0
            ? $"{Format.Duration(elapsed)} / ~{Format.Duration(estimate)}"
            : Format.Duration(elapsed);
        var prefix = process is null ? "" : $"{process.Name} {process.Pid} · ";
        var (chip, chipTooltip, benchmark) = ChipOf(lease);
        var tile = Format.Initials(process?.Name ?? FirstWord(lease.Label));

        if (lastKnown)
        {
            return new JobCard(
                tile,
                false,
                process?.Exe,
                chip,
                chipTooltip,
                benchmark,
                "",
                $"{prefix}was at {time}",
                "",
                false,
                "",
                false,
                progress,
                EdgeBarTone.Frozen,
                CardAction.None,
                "",
                false);
        }

        var detail = prefix + Format.Reservation(lease.Profile);
        var name = process?.Name ?? lease.Label;
        var card = new JobCard(
            tile,
            false,
            process?.Exe,
            chip,
            chipTooltip,
            benchmark,
            "",
            detail,
            detail,
            false,
            over ? $"{Format.Duration(elapsed - estimate)} over" : time,
            over,
            over ? 1 : progress,
            over ? EdgeBarTone.Over : EdgeBarTone.Running,
            CardAction.Kill,
            $"Kill {name}",
            false);
        if (EndedOutside(card, stop, started, estimate, nowMs) is { } outside)
        {
            return outside;
        }

        if (stop is not { Kind: StopKind.Kill })
        {
            return card;
        }

        switch (stop.Phase)
        {
            case StopPhase.Confirming:
                var confirm = $"Stops {process?.Name ?? "the command"} and its child processes";
                return card with
                {
                    Detail = confirm,
                    DetailTooltip = confirm,
                    DetailIsDanger = true,
                    Time = $"{stop.SecondsLeft}s",
                    TimeIsWarning = false,
                    Action = CardAction.Confirm,
                    ActionName = $"Confirm kill {name}",
                };
            case StopPhase.Sending:
                return card with
                {
                    Detail = process is null ? "Stopping…" : $"Stopping {process.Name}…",
                    DetailTooltip = "",
                    Time = "",
                    TimeIsWarning = false,
                    Action = CardAction.None,
                    ActionName = "",
                };
            case StopPhase.Failed:
                return card with
                {
                    Detail = $"Not stopped: {stop.Message}",
                    DetailTooltip = $"Not stopped: {stop.Message}",
                    DetailIsDanger = true,
                    Announcement = $"{name} not stopped: {stop.Message}",
                };
            default:
                var ran = Seconds((stop.EndedAtMs ?? nowMs) - started);
                return card with
                {
                    Strong = "Stopped",
                    Detail = Notified(stop.Message),
                    DetailTooltip = "",
                    Time = $"at {Format.Duration(ran)}",
                    TimeIsWarning = false,
                    BarFraction = estimate > 0 ? Fraction(ran, estimate) : 0,
                    BarTone = EdgeBarTone.Frozen,
                    Action = CardAction.None,
                    ActionName = "",
                    IsEnded = true,
                    IsStruck = true,
                    Announcement = $"Stopped {name}{Notified(stop.Message)}",
                };
        }
    }

    /// <param name="job">The waiting request.</param>
    /// <param name="nowMs">The moment to show, in the daemon's clock.</param>
    /// <param name="afterBenchmark">A benchmark runs or waits ahead of it.</param>
    /// <param name="lastKnown">The job comes from the last state allotd served before it went
    /// away.</param>
    /// <param name="stop">A Remove under way or done in this flyout, or <c>null</c>; ignored
    /// for last known state.</param>
    /// <param name="othersActive">Another lease is active, which a queued benchmark drains.</param>
    public static JobCard ForQueued(
        QueuedJob job,
        long nowMs,
        bool afterBenchmark,
        bool lastKnown,
        StopView? stop = null,
        bool othersActive = false)
    {
        var card = QueuedCard(job, nowMs, afterBenchmark, lastKnown, stop);
        return card.IsBenchmark
            ? Benchmark(
                card,
                BenchmarkStatus.ForQueued(job, othersActive),
                lastKnown,
                stop is { Kind: StopKind.Remove } ? stop : null)
            : card;
    }

    private static JobCard QueuedCard(
        QueuedJob job,
        long nowMs,
        bool afterBenchmark,
        bool lastKnown,
        StopView? stop)
    {
        var lease = job.Lease;
        var waited = Seconds(nowMs - lease.RequestedAtMs);
        var left = job.Verdict.WaitEstimateSecs;
        var (chip, chipTooltip, benchmark) = ChipOf(lease);

        string time;
        double fraction;
        var tone = EdgeBarTone.Waiting;
        if (afterBenchmark)
        {
            (time, fraction, tone) =
                ($"{Format.Duration(waited)} · after benchmark", 1, EdgeBarTone.WaitingDashed);
        }
        else if (job.Position == 1 && (job.Verdict.Fits || left <= StartsNextSecs))
        {
            time = left is { } soon ? $"starts next · ~{Format.Minutes(soon)}" : "starts next";
            fraction = left is { } rest ? Fraction(waited, waited + (long)rest) : 1;
        }
        else if (left is { } rest)
        {
            var total = waited + (long)rest;
            time = $"{Format.Duration(waited)} / ~{Format.Minutes((ulong)total)}";
            fraction = Fraction(waited, total);
        }
        else
        {
            (time, fraction, tone) =
                ($"{Format.Duration(waited)} · depends on queue", 1, EdgeBarTone.WaitingDashed);
        }

        var card = new JobCard(
            $"#{job.Position}",
            true,
            null,
            chip,
            chipTooltip,
            benchmark,
            "",
            Format.Reservation(lease.Profile),
            "",
            false,
            time,
            true,
            fraction,
            lastKnown ? EdgeBarTone.Frozen : tone,
            lastKnown ? CardAction.None : CardAction.Remove,
            lastKnown ? "" : $"Remove {lease.Label} from queue",
            false);
        if (lastKnown || stop is not { Kind: StopKind.Remove })
        {
            return card;
        }

        return stop.Phase switch
        {
            StopPhase.Sending => card with
            {
                Detail = "Removing…",
                Action = CardAction.None,
                ActionName = "",
            },
            StopPhase.Failed => card with
            {
                Detail = $"Not removed: {stop.Message}",
                DetailTooltip = $"Not removed: {stop.Message}",
                DetailIsDanger = true,
                Announcement = $"{lease.Label} not removed: {stop.Message}",
            },
            StopPhase.Ended => card with
            {
                Strong = "Removed",
                Detail = Notified(stop.Message),
                Time = "",
                TimeIsWarning = false,
                BarFraction = 0,
                BarTone = EdgeBarTone.None,
                Action = CardAction.None,
                ActionName = "",
                IsEnded = true,
                IsStruck = true,
                Announcement = $"Removed {lease.Label}{Notified(stop.Message)}",
            },
            _ => card,
        };
    }

    // A benchmark shows its progress on the stepper instead of an edge bar, and its status line
    // instead of the reservation. While a stop is under way the stepper stays and the line is
    // the stop's; once stopped or removed, and in last known state, only the plain line is left,
    // without a time, as in the prototype.
    private static JobCard Benchmark(
        JobCard card,
        BenchmarkStatus status,
        bool lastKnown,
        StopView? stop)
    {
        card = card with { BarFraction = 0, BarTone = EdgeBarTone.None };
        if (card.IsEnded)
        {
            return card with { Time = "" };
        }

        if (lastKnown)
        {
            return card;
        }

        if (stop is not null)
        {
            return card with
            {
                Time = stop.Phase == StopPhase.Confirming ? card.Time : "",
                TimeIsWarning = false,
                Steps = status.Steps,
            };
        }

        return card with
        {
            Detail = status.Text,
            DetailTooltip = "",
            Time = "",
            TimeIsWarning = false,
            Icon = status.Icon,
            Steps = status.Steps,
        };
    }

    /// <summary>
    /// A job that ended without this flyout stopping it, held like Stopped:
    /// Finished with the green check and its run time, not struck through; a kill from
    /// elsewhere reads as a stop; allotd's own end gives its reason.
    /// </summary>
    /// <returns>The held card, or <c>null</c> when <paramref name="stop"/> is no such
    /// end.</returns>
    private static JobCard? EndedOutside(
        JobCard card,
        StopView? stop,
        long started,
        long estimate,
        long nowMs)
    {
        if (stop is not { Phase: StopPhase.Ended }
            || stop.Kind is not (StopKind.Finished or StopKind.KilledOutside
                or StopKind.EndedByRhd))
        {
            return null;
        }

        var ran = Seconds((stop.EndedAtMs ?? nowMs) - started);
        var ended = card with
        {
            DetailTooltip = "",
            Time = $"at {Format.Duration(ran)}",
            TimeIsWarning = false,
            BarFraction = estimate > 0 ? Fraction(ran, estimate) : 0,
            BarTone = EdgeBarTone.Frozen,
            Action = CardAction.None,
            ActionName = "",
            IsEnded = true,
        };
        return stop.Kind switch
        {
            StopKind.Finished => ended with
            {
                Strong = "Finished",
                Detail = $" · {Format.Duration(ran)}",
                Time = "",
                Icon = DetailIcon.Check,
            },
            StopKind.KilledOutside => ended with
            {
                Strong = "Stopped outside the flyout",
                Detail = Notified(""),
                IsStruck = true,
            },
            _ => ended with
            {
                Strong = "Ended by allotd",
                Detail = stop.Message.Length == 0 ? "" : $" · {stop.Message}",
                DetailTooltip = stop.Message,
            },
        };
    }

    private static string Notified(string note) =>
        note.Length == 0 ? " · Claude was notified" : $" · {note} · Claude was notified";

    private static (string Label, string Tooltip, bool IsBenchmark) ChipOf(Lease lease)
    {
        if (lease.Kind == LeaseKind.BenchmarkExclusive)
        {
            return ("Benchmark", "Benchmark", true);
        }

        var label = lease.Class switch
        {
            JobClass.CpuHeavy => "CPU heavy",
            JobClass.GpuHeavy => "GPU heavy",
            JobClass.Mixed => "Mixed",
            JobClass.Light => "Light",
            _ => "Custom",
        };
        return (label, lease.Class is null ? "Custom reservation" : label, false);
    }

    private static long Seconds(long milliseconds) => Math.Max(0, milliseconds / 1000);

    private static double Fraction(long part, long whole) =>
        whole <= 0 ? 1 : Math.Clamp((double)part / whole, 0, 1);

    private static string FirstWord(string command)
    {
        var trimmed = command.TrimStart();
        var end = trimmed.IndexOf(' ', StringComparison.Ordinal);
        return end < 0 ? trimmed : trimmed[..end];
    }
}
