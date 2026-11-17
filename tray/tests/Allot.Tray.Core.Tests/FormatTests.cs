using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class FormatTests
{
    [TestMethod]
    [DataRow(@"E:\code\allot", "allot")]
    [DataRow(@"E:\code\ml-trainer\", "ml-trainer")]
    [DataRow("/home/someone/ingest-svc", "ingest-svc")]
    [DataRow("parser-perf", "parser-perf")]
    [DataRow("", "")]
    [DataRow(null, "")]
    public void ProjectIsTheLastFolder(string? cwd, string expected) =>
        Assert.AreEqual(expected, Format.Project(cwd));

    [TestMethod]
    [DataRow("3f2a9c1e-77b0-4f6e-a2d8-5c9e1b4f7a02", "3f2a9c1e")]
    [DataRow("short", "short")]
    public void ShortIdIsTheFirstEightCharacters(string session, string expected) =>
        Assert.AreEqual(expected, Format.ShortId(session));

    [TestMethod]
    public void UsageIgnoresTheCurrentCulture()
    {
        var usage = new ResourceUsage { Cores = 7.84f, GpuPct = 0, RamMb = 2970, VramMb = 0 };
        var previous = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("de-DE");
        try
        {
            Assert.AreEqual("7.8 cores · 2.9 GB", Format.Usage(usage));
        }
        finally
        {
            CultureInfo.CurrentCulture = previous;
        }
    }

    [TestMethod]
    [DataRow(0UL, "0.0")]
    [DataRow(307UL, "0.3")]
    [DataRow(32768UL, "32.0")]
    public void GigabytesDivideBy1024(ulong megabytes, string expected) =>
        Assert.AreEqual(expected, Format.Gigabytes(megabytes));

    [TestMethod]
    [DataRow(0L, "0:00")]
    [DataRow(186L, "3:06")]
    [DataRow(3599L, "59:59")]
    [DataRow(3723L, "1:02:03")]
    [DataRow(-5L, "0:00")]
    public void DurationIsMinutesAndSecondsThenHours(long seconds, string expected) =>
        Assert.AreEqual(expected, Format.Duration(seconds));

    [TestMethod]
    [DataRow(0UL, "1 min")]
    [DataRow(29UL, "1 min")]
    [DataRow(90UL, "2 min")]
    [DataRow(120UL, "2 min")]
    [DataRow(3600UL, "1 h")]
    [DataRow(4800UL, "1 h 20 min")]
    public void MinutesRoundAndNeverReadZero(ulong seconds, string expected) =>
        Assert.AreEqual(expected, Format.Minutes(seconds));

    [TestMethod]
    [DataRow(0L, "just now")]
    [DataRow(59L, "just now")]
    [DataRow(240L, "4 min ago")]
    [DataRow(7200L, "2 h ago")]
    [DataRow(259_200L, "3 d ago")]
    public void AgoGrowsFromMinutesToDays(long seconds, string expected) =>
        Assert.AreEqual(expected, Format.Ago(seconds));

    [TestMethod]
    [DataRow(1f, "1 core")]
    [DataRow(8f, "8 cores")]
    [DataRow(2.5f, "2.5 cores")]
    public void CoresAreSingularForOne(float cores, string expected) =>
        Assert.AreEqual(expected, Format.Cores(cores));

    [TestMethod]
    public void ReservationLeavesOutWhatIsNotReserved()
    {
        var profile = new JobProfile { Cores = 2, EstSecs = 60, GpuPct = 0, RamMb = 0, VramMb = 0 };

        Assert.AreEqual("2 cores", Format.Reservation(profile));
        Assert.AreEqual(
            "2 cores · 1.5 GB · GPU 80% · 4 GB VRAM",
            Format.Reservation(profile with { RamMb = 1536, GpuPct = 80, VramMb = 4096 }));
    }

    [TestMethod]
    [DataRow(5.6f, 8f, "5.6 of 8 cores")]
    [DataRow(0.5f, 1f, "0.5 of 1 cores")]
    [DataRow(8f, 8f, "8 cores")]
    [DataRow(7.96f, 8f, "8 cores")]
    [DataRow(1f, null, "1 core")]
    public void GrantedNamesTheAskedCoresOnlyWhenFewer(
        float granted,
        float? asked,
        string expected) =>
        Assert.AreEqual(expected, Format.Granted(granted, asked));

    [TestMethod]
    public void LeaseReservationShowsAShrunkGrant()
    {
        var lease = Handoff.State(run: ["cargo"]).Active[0].Lease;
        var shrunk = lease with
        {
            Profile = lease.Profile with { Cores = 5.6f, RamMb = 4096 },
            AskedCores = 8,
        };

        Assert.AreEqual("5.6 of 8 cores · 4 GB", Format.Reservation(shrunk));
        Assert.AreEqual(
            "5.6 cores · 4 GB",
            Format.Reservation(shrunk with { AskedCores = null }));
    }

    [TestMethod]
    public void StatsShowMedianAndMaxWithGpuOnlyWhenUsed()
    {
        var median = new JobProfile
        {
            Cores = 0.85f,
            EstSecs = 190,
            GpuPct = 0,
            RamMb = 1229,
            VramMb = 0,
        };
        var max = median with { Cores = 3.4f, EstSecs = 292, RamMb = 2048 };
        var stats = new RunStats { Runs = 12, Median = median, P90 = max, Max = max };

        Assert.AreEqual(
            "median 0.85 cores · 1.2 GB · 3:10   max 3.4 cores · 2 GB · 4:52",
            Format.Stats(stats));
        Assert.AreEqual(
            "median 1 core · 1.2 GB · GPU 0% · 0 GB VRAM · 3:10   "
                + "max 3.4 cores · 2 GB · GPU 40% · 1 GB VRAM · 4:52",
            Format.Stats(stats with
            {
                Median = median with { Cores = 1 },
                Max = max with { GpuPct = 40, VramMb = 1024 },
            }));
    }

    [TestMethod]
    [DataRow("cargo.exe", "CA")]
    [DataRow("go.exe", "GO")]
    [DataRow("NODE.EXE", "NO")]
    [DataRow(@"C:\tools\x.exe", "X")]
    [DataRow(@"""C:\Program Files\py\python.exe""", "PY")]
    [DataRow("7z", "7Z")]
    [DataRow("", "")]
    public void InitialsAreTheFirstTwoLettersOfTheProgram(string program, string expected) =>
        Assert.AreEqual(expected, Format.Initials(program));
}
