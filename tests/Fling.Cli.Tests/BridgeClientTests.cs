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

        var error = await Assert.ThrowsAsync<BridgeException>(() => client.StopAsync("abc", null, CancellationToken.None));

        Assert.Equal("bad request", error.Message);
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
