using Allot.Tray.Core.Flyout;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class CubicBezierTests
{
    [TestMethod]
    public void EndsAreFixed()
    {
        Assert.AreEqual(0, CubicBezier.Decelerate.Ease(0));
        Assert.AreEqual(1, CubicBezier.Decelerate.Ease(1));
        Assert.AreEqual(0, CubicBezier.Decelerate.Ease(-0.5));
        Assert.AreEqual(1, CubicBezier.Decelerate.Ease(1.5));
    }

    [TestMethod]
    public void LinearCurveIsTheIdentity()
    {
        var linear = new CubicBezier(0, 0, 1, 1);

        Assert.AreEqual(0.25, linear.Ease(0.25), 1e-6);
        Assert.AreEqual(0.7, linear.Ease(0.7), 1e-6);
    }

    [TestMethod]
    public void DecelerateCoversMostOfTheDistanceEarly()
    {
        // Worked by hand: x(t) = 0.25 at t = 0.515, where y(t) = 0.85.
        Assert.AreEqual(0.85, CubicBezier.Decelerate.Ease(0.25), 0.01);
        Assert.IsGreaterThan(0.95, CubicBezier.Decelerate.Ease(0.6));
    }

    [TestMethod]
    public void AccelerateCoversLittleOfTheDistanceEarly()
    {
        Assert.IsLessThan(0.05, CubicBezier.Accelerate.Ease(0.25));
        Assert.AreEqual(1, CubicBezier.Accelerate.Ease(1));
    }

    [TestMethod]
    public void EaseIsMonotonic()
    {
        var previous = 0.0;
        for (var i = 1; i <= 100; i++)
        {
            var value = CubicBezier.Decelerate.Ease(i / 100.0);
            Assert.IsGreaterThanOrEqualTo(previous, value);
            previous = value;
        }
    }
}
