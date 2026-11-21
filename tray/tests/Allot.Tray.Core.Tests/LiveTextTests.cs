using System.Globalization;
using System.Text.Json;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

namespace Allot.Tray.Core.Tests;

// The cases are contract/text-fixtures/live-text.json, which the GNOME and macOS tests run too, so
// every port reads the same value as the same text.
[TestClass]
public sealed class LiveTextTests
{
    [TestMethod]
    public void NumbersRoundHalfAwayFromZeroLikeTheOtherPanels()
    {
        var (number, document) = Load();
        using (document)
        {
            foreach (var entry in document.RootElement.GetProperty("numbers").EnumerateArray())
            {
                Assert.AreEqual(
                    entry.GetProperty("text").GetString(),
                    number(entry.GetProperty("value").GetDouble()));
            }
        }
    }

    [TestMethod]
    public void HeadroomReadsMemoryInGigabytesRoundedHalfAwayFromZero()
    {
        var (number, document) = Load();
        using (document)
        {
            foreach (var entry in document.RootElement.GetProperty("headroom").EnumerateArray())
            {
                var kept = entry.GetProperty("kept")
                    .Deserialize(ContractJsonContext.Default.KeptFree);
                Assert.IsNotNull(kept);
                Assert.AreEqual(
                    entry.GetProperty("text").GetString(), LiveText.Headroom(kept, number));
            }
        }
    }

    [TestMethod]
    public void EstimatesRoundTheReservedCoresHalfAwayFromZero()
    {
        var (number, document) = Load();
        using (document)
        {
            foreach (var entry in document.RootElement.GetProperty("estimates").EnumerateArray())
            {
                var margin = entry.GetProperty("margin_pct").GetDouble();
                Assert.AreEqual(
                    entry.GetProperty("text").GetString(),
                    LiveText.Estimates(true, 3, margin, number));
            }
        }
    }

    [TestMethod]
    public void TheFitsCaptionRoundsTheBusyShareHalfAwayFromZero()
    {
        var (number, document) = Load();
        using (document)
        {
            foreach (var entry in document.RootElement.GetProperty("fits_caption").EnumerateArray())
            {
                var busy = entry.GetProperty("cpu_busy_pct").GetDouble();
                Assert.AreEqual(
                    entry.GetProperty("text").GetString(), LiveText.FitsCaption(busy, number));
            }
        }
    }

    private static (Func<double, string> Number, JsonDocument Document) Load()
    {
        var document = JsonDocument.Parse(
            File.ReadAllText(
                Path.Combine(AppContext.BaseDirectory, "TextFixtures", "live-text.json")));
        var locale = document.RootElement.GetProperty("locale").GetString()!;
        return (SettingsText.Numbers(CultureInfo.GetCultureInfo(locale)), document);
    }
}
