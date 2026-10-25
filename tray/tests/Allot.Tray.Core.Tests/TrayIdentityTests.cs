namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class TrayIdentityTests
{
    [TestMethod]
    public void InstanceKeyStaysFixedAcrossReleases() =>
        Assert.AreEqual("Allot.Tray", TrayIdentity.InstanceKey);
}
