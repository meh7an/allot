namespace Allot.Tray.Flyout;

/// <summary>
/// What the flyout window needs from its content.
/// </summary>
public interface IFlyoutContent
{
    /// <summary>
    /// The height the content needs without scrolling, in effective pixels. The window clamps it
    /// to 640 or 70% of the work area.
    /// </summary>
    double DesiredHeight { get; }

    /// <summary>
    /// Raised whenever <see cref="DesiredHeight"/> changes, including on every frame of a card's
    /// enter or exit animation.
    /// </summary>
    event Action? DesiredHeightChanged;

    /// <summary>
    /// Moves focus to the first control when the flyout opens.
    /// </summary>
    void FocusFirst();
}
