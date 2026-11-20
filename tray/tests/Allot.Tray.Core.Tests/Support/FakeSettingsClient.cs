using System.Text.Json;
using System.Text.Json.Nodes;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;

namespace Allot.Tray.Core.Tests.Support;

/// <summary>
/// A settings client whose answers each test queues; it records every patch and preview.
/// </summary>
internal sealed class FakeSettingsClient : ISettingsClient
{
    private readonly Queue<Action> _inFlight = new();

    public Queue<SettingsReply<SettingsState>> Reads { get; } = new();

    public Queue<SettingsReply<SettingsState>> Writes { get; } = new();

    public List<SettingsPatch> Patches { get; } = [];

    public List<IReadOnlyList<Change>> Previews { get; } = [];

    public SettingsState State { get; set; } = FixtureState();

    /// <summary>
    /// The settings-state fixture, as <c>GET /settings</c> answers it.
    /// </summary>
    public static SettingsState FixtureState() => Decode(Fixtures.Read("settings-state.json"));

    /// <summary>
    /// The fixture's state with a preset of the user's own, <c>render</c>.
    /// </summary>
    public static SettingsState StateWithRender()
    {
        var root = JsonNode.Parse(Fixtures.Read("settings-state.json"))!.AsObject();
        var presets = root["values"]!.AsArray()
            .Single(value => value!["key"]!.GetValue<string>() == "presets")!;
        presets["value"]!.AsArray().Add(JsonNode.Parse(
            """
            {"name":"render","profile":{"cores":6,"ram_mb":8192,"gpu_pct":40,"vram_mb":4096,
            "est_secs":900}}
            """));
        return Decode(root.ToJsonString());
    }

    public static SettingsReply<T> Failure<T>(
        SettingsFailureKind kind,
        string? message = null,
        IReadOnlyList<FieldError>? problems = null,
        string? revision = null)
        where T : class =>
        new(new SettingsFailure(kind, message ?? kind.ToString(), problems ?? [], revision));

    public Task<SettingsReply<SettingsState>> SettingsAsync(CancellationToken cancellationToken)
    {
        var reply = Reads.TryDequeue(out var queued)
            ? queued
            : new SettingsReply<SettingsState>(State);
        return Task.FromResult(reply);
    }

    /// <summary>
    /// A patch stays in flight, as a request over HTTP does, until <see cref="CompleteAll"/>; an
    /// edit made meanwhile meets the session's one-request-in-flight rule.
    /// </summary>
    public Task<SettingsReply<SettingsState>> ChangeSettingsAsync(
        SettingsPatch patch,
        CancellationToken cancellationToken)
    {
        Patches.Add(patch);
        var reply = Writes.TryDequeue(out var queued)
            ? queued
            : new SettingsReply<SettingsState>(State);
        var source = new TaskCompletionSource<SettingsReply<SettingsState>>();
        _inFlight.Enqueue(() => source.SetResult(reply));
        return source.Task;
    }

    /// <summary>
    /// Answers every patch in flight, and those the answers send in turn.
    /// </summary>
    public void CompleteAll()
    {
        while (_inFlight.TryDequeue(out var answer))
        {
            answer();
        }
    }

    public Task<SettingsReply<FitPreview>> PreviewSettingsAsync(
        IReadOnlyList<Change> changes,
        CancellationToken cancellationToken)
    {
        Previews.Add(changes);
        return Task.FromResult(Failure<FitPreview>(SettingsFailureKind.Unavailable));
    }

    public Task<SettingsReply<CommandTest>> ClassifyAsync(
        ClassifyRequest request,
        CancellationToken cancellationToken) =>
        Task.FromResult(Failure<CommandTest>(SettingsFailureKind.Unavailable));

    private static SettingsState Decode(string json) =>
        JsonSerializer.Deserialize(json, ContractJsonContext.Default.SettingsState)!;
}
