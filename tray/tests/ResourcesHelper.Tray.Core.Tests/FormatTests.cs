using System.Globalization;

using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class FormatTests
{
    [TestMethod]
    [DataRow(@"E:\code\resources-helper-mcp", "resources-helper-mcp")]
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
}
