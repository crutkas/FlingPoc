using System.Net;
using System.Net.Http.Json;
using System.Text.Json;

namespace Fling.Cli;

public sealed class BridgeClient(HttpClient httpClient)
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public async Task<IReadOnlyList<Device>> GetDevicesAsync(CancellationToken cancellationToken) =>
        await SendAsync<IReadOnlyList<Device>>(HttpMethod.Get, "/v1/devices", null, cancellationToken);

    public Task<BridgeCapabilities> GetCapabilitiesAsync(CancellationToken cancellationToken) =>
        SendAsync<BridgeCapabilities>(HttpMethod.Get, "/v1/capabilities", null, cancellationToken);

    public Task<PairingSession> StartPairingAsync(string deviceId, CancellationToken cancellationToken) =>
        SendAsync<PairingSession>(HttpMethod.Post, "/v1/pair/start", new { deviceId }, cancellationToken);

    public Task FinishPairingAsync(string sessionId, string pin, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/pair/finish", new { sessionId, pin }, cancellationToken);

    public Task PlayUrlAsync(string deviceId, string url, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/play", new { deviceId, url }, cancellationToken);

    public Task PlayFileAsync(string deviceId, string path, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/file", new { deviceId, path }, cancellationToken);

    public Task StartMirrorAsync(string deviceId, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/mirror", new { deviceId }, cancellationToken);

    public Task<PlaybackStatus> GetStatusAsync(string deviceId, CancellationToken cancellationToken) =>
        SendAsync<PlaybackStatus>(HttpMethod.Post, "/v1/status", new { deviceId }, cancellationToken);

    public Task StopAsync(string deviceId, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/stop", new { deviceId }, cancellationToken);

    public async Task<bool> IsHealthyAsync(CancellationToken cancellationToken)
    {
        try
        {
            using var response = await httpClient.GetAsync("/health", cancellationToken);
            return response.IsSuccessStatusCode;
        }
        catch (HttpRequestException)
        {
            return false;
        }
    }

    private async Task SendAsync(HttpMethod method, string path, object? body, CancellationToken cancellationToken) =>
        await SendAsync<object?>(method, path, body, cancellationToken);

    private async Task<T> SendAsync<T>(HttpMethod method, string path, object? body, CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(method, path);
        if (body is not null)
        {
            request.Content = JsonContent.Create(body);
        }
        using var response = await httpClient.SendAsync(request, cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            var error = await response.Content.ReadFromJsonAsync<ErrorResponse>(JsonOptions, cancellationToken);
            throw new BridgeException(response.StatusCode, error?.Error ?? response.ReasonPhrase ?? "Unknown bridge error");
        }
        if (typeof(T) == typeof(object))
        {
            return default!;
        }
        return (await response.Content.ReadFromJsonAsync<T>(JsonOptions, cancellationToken))!;
    }
}

public sealed record Device(string Id, string Name, string Address, IReadOnlyList<string> Protocols);
public sealed record PairingSession(string SessionId);
public sealed record PlaybackStatus(string State, string? Title, double? Position, double? Duration);
public sealed record BridgeCapability(string Status, string Detail);
public sealed record BridgeCapabilities(
    BridgeCapability Discovery,
    BridgeCapability Pairing,
    BridgeCapability UrlPlayback,
    BridgeCapability FilePlayback,
    BridgeCapability PlaybackStatus,
    BridgeCapability Stop,
    BridgeCapability HlsMirroring,
    BridgeCapability NativeMirroring);
internal sealed record ErrorResponse(string Error);

public sealed class BridgeException(HttpStatusCode statusCode, string message) : Exception(message)
{
    public HttpStatusCode StatusCode { get; } = statusCode;
}
