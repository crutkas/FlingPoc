using System.Net;
using System.Net.Http.Json;
using System.Text.Json;

namespace Fling.Cli;

public sealed class BridgeClient(HttpClient httpClient)
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public async Task<IReadOnlyList<Device>> GetDevicesAsync(CancellationToken cancellationToken) =>
        await SendAsync<IReadOnlyList<Device>>(HttpMethod.Get, "/v1/devices", null, cancellationToken);

    public Task<PairingSession> StartPairingAsync(string deviceId, CancellationToken cancellationToken) =>
        SendAsync<PairingSession>(HttpMethod.Post, "/v1/pair/start", new { deviceId }, cancellationToken);

    public Task<PairingResult> FinishPairingAsync(string sessionId, string pin, CancellationToken cancellationToken) =>
        SendAsync<PairingResult>(HttpMethod.Post, "/v1/pair/finish", new { sessionId, pin }, cancellationToken);

    public Task PlayUrlAsync(string deviceId, string url, string? credentials, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/play", new { deviceId, url, credentials }, cancellationToken);

    public Task PlayFileAsync(string deviceId, string path, string? credentials, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/file", new { deviceId, path, credentials }, cancellationToken);

    public Task StartMirrorAsync(string deviceId, string? display, string? credentials, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/mirror", new { deviceId, display, credentials }, cancellationToken);

    public Task StopAsync(string deviceId, string? credentials, CancellationToken cancellationToken) =>
        SendAsync(HttpMethod.Post, "/v1/stop", new { deviceId, credentials }, cancellationToken);

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
public sealed record PairingResult(string Credentials);
internal sealed record ErrorResponse(string Error);

public sealed class BridgeException(HttpStatusCode statusCode, string message) : Exception(message)
{
    public HttpStatusCode StatusCode { get; } = statusCode;
}
