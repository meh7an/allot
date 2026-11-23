using System.Net;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class SettingsClientTests
{
    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task SettingsStateFixtureDecodesWithEverySettingsValue()
    {
        using var directory = new TempDirectory();
        var sent = new List<SentRequest>();
        using var client = Client(directory, sent, () => Json(Fixtures.Read("settings-state.json")));

        var reply = await client.SettingsAsync(TestContext.CancellationToken);

        Assert.IsNotNull(reply.Value);
        Assert.AreEqual("69fd99abe4f654c7", reply.Value.Revision);
        Assert.HasCount(1, reply.Value.PendingRestart);
        Assert.AreEqual("daemon.port", reply.Value.PendingRestart[0]);
        Assert.AreEqual(47_823u, reply.Value.ListeningPort);
        Assert.IsNull(reply.Value.FileError);
        Assert.HasCount(24, reply.Value.Values);
        var request = sent.Single();
        Assert.AreEqual(HttpMethod.Get, request.Method);
        Assert.AreEqual("/settings", request.Path);
        Assert.IsNull(request.Body);
    }

    [TestMethod]
    public async Task PatchIsSentAsTheSettingsPatchFixtureEncodesIt()
    {
        using var directory = new TempDirectory();
        var sent = new List<SentRequest>();
        using var client = Client(directory, sent, () => Json(Fixtures.Read("settings-state.json")));
        var patch = Decode(
            Fixtures.Read("settings-patch.json"), ContractJsonContext.Default.SettingsPatch);

        var reply = await client.ChangeSettingsAsync(patch, TestContext.CancellationToken);

        Assert.IsNotNull(reply.Value);
        var request = sent.Single();
        Assert.AreEqual(HttpMethod.Patch, request.Method);
        Assert.AreEqual("/settings", request.Path);
        Assert.AreEqual("application/json", request.MediaType);
        // The fixture writes 6.0 where the generated types write 6, so the body is compared with
        // the fixture as the types encode it, and then checked against the fixture's shape.
        Assert.AreEqual(
            JsonSerializer.Serialize(patch, ContractJsonContext.Default.SettingsPatch),
            request.Body);
        var body = JsonNode.Parse(request.Body!)!.AsObject();
        Assert.AreEqual("69fd99abe4f654c7", body["revision"]!.GetValue<string>());
        Assert.AreEqual(7, body["changes"]!.AsArray().Count);
    }

    [TestMethod]
    public async Task CommandTestSendsTheClassifyRequestFixtureAndDecodesTheAnswer()
    {
        using var directory = new TempDirectory();
        var sent = new List<SentRequest>();
        using var client = Client(directory, sent, () => Json(Fixtures.Read("command-test.json")));
        var classify = Decode(
            Fixtures.Read("classify-request.json"), ContractJsonContext.Default.ClassifyRequest);

        var reply = await client.ClassifyAsync(classify, TestContext.CancellationToken);

        Assert.IsNotNull(reply.Value);
        CollectionAssert.AreEqual(
            new List<CommandKind> { CommandKind.Heavy, CommandKind.PrintOnly, CommandKind.NotHeavy },
            reply.Value.Commands.Select(command => command.Kind).ToList());
        Assert.AreEqual(JobClass.CpuHeavy, reply.Value.Lease?.Class);
        var request = sent.Single();
        Assert.AreEqual(HttpMethod.Post, request.Method);
        Assert.AreEqual("/classify", request.Path);
        AssertSameJson(Fixtures.Read("classify-request.json"), request.Body);
    }

    [TestMethod]
    public async Task PreviewSendsTheUnsavedChangesAndDecodesWhatFits()
    {
        using var directory = new TempDirectory();
        var sent = new List<SentRequest>();
        using var client = Client(directory, sent, () => Json(Fixtures.Read("fit-preview.json")));
        IReadOnlyList<Change> changes =
        [
            new Change.SetNumber { Key = "admission.headroom_pct", Value = 30 },
        ];

        var reply = await client.PreviewSettingsAsync(changes, TestContext.CancellationToken);

        Assert.IsNotNull(reply.Value);
        Assert.AreEqual(16u, reply.Value.KeptFree.TotalCores);
        var request = sent.Single();
        Assert.AreEqual(HttpMethod.Post, request.Method);
        Assert.AreEqual("/settings/preview", request.Path);
        AssertSameJson(
            """{"changes":[{"kind":"set_number","key":"admission.headroom_pct","value":30}]}""",
            request.Body);
    }

    [TestMethod]
    public async Task RefusalsKeepAllotdsMessageAndProblems()
    {
        await AssertRefusalAsync(
            HttpStatusCode.Conflict,
            "settings-refusal-conflict.json",
            SettingsFailureKind.Conflict);
        await AssertRefusalAsync(
            HttpStatusCode.UnprocessableEntity,
            "settings-refusal-invalid.json",
            SettingsFailureKind.Invalid);
    }

    [TestMethod]
    public async Task EachOtherAnswerHasItsOwnKind()
    {
        const string held = """{"message":"config.toml is open","problems":[]}""";
        List<Answer> cases =
        [
            new((HttpStatusCode)423, held, SettingsFailureKind.Held, "config.toml is open"),
            new(
                HttpStatusCode.ServiceUnavailable,
                string.Empty,
                SettingsFailureKind.Unavailable,
                "the service answered 503"),
            new(
                HttpStatusCode.NotFound,
                string.Empty,
                SettingsFailureKind.Outdated,
                "the service answered 404"),
            new(
                HttpStatusCode.InternalServerError,
                "internal error\n",
                SettingsFailureKind.Failed,
                "internal error"),
            new(
                HttpStatusCode.Unauthorized,
                string.Empty,
                SettingsFailureKind.Token,
                "the service refused the token"),
        ];
        foreach (var answer in cases)
        {
            using var directory = new TempDirectory();
            using var client = Client(directory, [], () => Json(answer.Body, answer.Status));

            var reply = await client.SettingsAsync(TestContext.CancellationToken);

            Assert.IsNotNull(reply.Failure);
            Assert.AreEqual(answer.Kind, reply.Failure.Kind);
            Assert.AreEqual(answer.Message, reply.Failure.Message);
        }
    }

    [TestMethod]
    public async Task AnAnswerThatIsNotASettingsStateFails()
    {
        using var directory = new TempDirectory();
        using var client = Client(directory, [], () => Json("""{"spec": 1}"""));

        var reply = await client.SettingsAsync(TestContext.CancellationToken);

        Assert.IsNotNull(reply.Failure);
        Assert.AreEqual(SettingsFailureKind.Failed, reply.Failure.Kind);
    }

    [TestMethod]
    public async Task NoTokenAndNoAnswerAreToldApart()
    {
        using var noToken = new TempDirectory();
        var sentWithoutToken = new List<SentRequest>();
        using var withoutToken = Client(noToken, sentWithoutToken, () => Json(string.Empty), null);
        using var down = new TempDirectory();
        using var refused = Client(down, [], () => throw FakeHttpHandler.Refused());

        var missing = await withoutToken.SettingsAsync(TestContext.CancellationToken);
        var unreachable = await refused.SettingsAsync(TestContext.CancellationToken);

        Assert.IsNotNull(missing.Failure);
        Assert.AreEqual(SettingsFailureKind.Token, missing.Failure.Kind);
        Assert.IsEmpty(sentWithoutToken);
        Assert.IsNotNull(unreachable.Failure);
        Assert.AreEqual(SettingsFailureKind.Unreachable, unreachable.Failure.Kind);
        StringAssert.Contains(unreachable.Failure.Message, "actively refused");
    }

    [TestMethod]
    public async Task ACancelThrowsInsteadOfReadingAsUnreachable()
    {
        using var directory = new TempDirectory();
        using var cancelled = new CancellationTokenSource();
        await cancelled.CancelAsync();
        using var client = Client(
            directory, [], () => throw new OperationCanceledException(cancelled.Token));

        await Assert.ThrowsAsync<OperationCanceledException>(
            () => client.SettingsAsync(cancelled.Token));
    }

    private async Task AssertRefusalAsync(
        HttpStatusCode status,
        string file,
        SettingsFailureKind kind)
    {
        using var directory = new TempDirectory();
        var json = Fixtures.Read(file);
        var refusal = Decode(json, ContractJsonContext.Default.SettingsRefusal);
        using var client = Client(directory, [], () => Json(json, status));

        var reply = await client.SettingsAsync(TestContext.CancellationToken);

        Assert.IsNull(reply.Value);
        Assert.IsNotNull(reply.Failure);
        Assert.AreEqual(kind, reply.Failure.Kind);
        Assert.AreEqual(refusal.Message, reply.Failure.Message);
        CollectionAssert.AreEqual(refusal.Problems.ToList(), reply.Failure.Problems.ToList());
    }

    private static DaemonClient Client(
        TempDirectory directory,
        List<SentRequest> sent,
        Func<HttpResponseMessage> respond,
        string? token = "t")
    {
        var handler = new FakeHttpHandler(request =>
        {
            sent.Add(new SentRequest(
                request.Method,
                request.RequestUri!.AbsolutePath,
                FakeHttpHandler.BodyOf(request),
                request.Content?.Headers.ContentType?.MediaType));
            return respond();
        });
        return DaemonClientTests.Client(directory, handler, token);
    }

    private static HttpResponseMessage Json(
        string json,
        HttpStatusCode status = HttpStatusCode.OK) =>
        FakeHttpHandler.Json(json, status);

    private static T Decode<T>(string json, JsonTypeInfo<T> type)
        where T : class
    {
        var decoded = JsonSerializer.Deserialize(json, type);
        Assert.IsNotNull(decoded);
        return decoded;
    }

    private static void AssertSameJson(string expected, string? actual)
    {
        Assert.IsNotNull(actual);
        Assert.IsTrue(
            JsonNode.DeepEquals(JsonNode.Parse(expected), JsonNode.Parse(actual)),
            $"{actual} is not {expected}");
    }

    private sealed record Answer(
        HttpStatusCode Status,
        string Body,
        SettingsFailureKind Kind,
        string Message);

    private sealed record SentRequest(
        HttpMethod Method,
        string Path,
        string? Body,
        string? MediaType);
}
