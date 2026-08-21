using System.Net;
using System.Text;

namespace Fling.Cli.Tests;

public sealed class BridgeClientTests
{
    [Fact]
    public async Task GetDevicesParsesBridgeResponse()
    {
        var handler = new StubHandler(HttpStatusCode.OK, """[{"id":"abc","name":"Living Room","address":"192.0.2.1","protocols":["AirPlay"]}]""");
        var client = new BridgeClient(new HttpClient(handler) { BaseAddress = new Uri("http://localhost") });

        var devices = await client.GetDevicesAsync(CancellationToken.None);

        var device = Assert.Single(devices);
        Assert.Equal("Living Room", device.Name);
    }

    [Fact]
    public async Task ErrorResponseBecomesBridgeException()
    {
        var handler = new StubHandler(HttpStatusCode.BadRequest, """{"error":"bad request"}""");
        var client = new BridgeClient(new HttpClient(handler) { BaseAddress = new Uri("http://localhost") });

        var error = await Assert.ThrowsAsync<BridgeException>(() => client.StopAsync("abc", CancellationToken.None));

        Assert.Equal("bad request", error.Message);
    }

    [Fact]
    public async Task CapabilitiesParseExplicitNativeMirroringBoundary()
    {
        var handler = new StubHandler(HttpStatusCode.OK, """
            {
              "discovery":{"status":"supported","detail":"mDNS"},
              "pairing":{"status":"pythonFallback","detail":"Python"},
              "urlPlayback":{"status":"pythonFallback","detail":"Python"},
              "filePlayback":{"status":"pythonFallback","detail":"Python"},
              "playbackStatus":{"status":"pythonFallback","detail":"Python"},
              "stop":{"status":"pythonFallback","detail":"Python"},
              "hlsMirroring":{"status":"pythonFallback","detail":"HLS"},
              "nativeMirroring":{"status":"unsupported","detail":"FairPlay"}
            }
            """);
        var client = new BridgeClient(new HttpClient(handler) { BaseAddress = new Uri("http://localhost") });

        var capabilities = await client.GetCapabilitiesAsync(CancellationToken.None);

        Assert.Equal("pythonFallback", capabilities.HlsMirroring.Status);
        Assert.Equal("unsupported", capabilities.NativeMirroring.Status);
    }

    private sealed class StubHandler(HttpStatusCode statusCode, string content) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(statusCode)
            {
                Content = new StringContent(content, Encoding.UTF8, "application/json"),
            });
    }
}
