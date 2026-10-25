using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Flyout;

/// <summary>
/// A button that tells assistive tech whether what it opens is expanded (handoff, Keyboard): the
/// Resources card's chevron and More options. A plain Button reports no such state.
/// </summary>
public sealed partial class ExpandButton : Button
{
    public static readonly DependencyProperty IsExpandedProperty = DependencyProperty.Register(
        nameof(IsExpanded),
        typeof(bool),
        typeof(ExpandButton),
        new PropertyMetadata(false, OnIsExpandedChanged));

    public bool IsExpanded
    {
        get => (bool)GetValue(IsExpandedProperty);
        set => SetValue(IsExpandedProperty, value);
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new ExpandButtonPeer(this);

    private static void OnIsExpandedChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs args)
    {
        if (FrameworkElementAutomationPeer.FromElement((UIElement)sender) is ExpandButtonPeer peer)
        {
            peer.RaiseExpandedChanged((bool)args.OldValue, (bool)args.NewValue);
        }
    }

    private sealed partial class ExpandButtonPeer(ExpandButton owner)
        : ButtonAutomationPeer(owner), IExpandCollapseProvider
    {
        public ExpandCollapseState ExpandCollapseState => StateOf(Button.IsExpanded);

        private ExpandButton Button => (ExpandButton)Owner;

        public void Expand()
        {
            if (!Button.IsExpanded)
            {
                Invoke();
            }
        }

        public void Collapse()
        {
            if (!Button.IsExpanded)
            {
                return;
            }

            // An open menu covers its button, so it is closed directly.
            if (Button.Flyout is { } flyout)
            {
                flyout.Hide();
            }
            else
            {
                Invoke();
            }
        }

        public void RaiseExpandedChanged(bool old, bool now) =>
            RaisePropertyChangedEvent(
                ExpandCollapsePatternIdentifiers.ExpandCollapseStateProperty,
                StateOf(old),
                StateOf(now));

        protected override object GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.ExpandCollapse
                ? this
                : base.GetPatternCore(patternInterface);

        private static ExpandCollapseState StateOf(bool expanded) =>
            expanded ? ExpandCollapseState.Expanded : ExpandCollapseState.Collapsed;
    }
}
