using System.Diagnostics;
using System.Net.Http.Headers;
using System.Security.Cryptography;

namespace Fling.Cli;

public sealed class BridgeProcess : IAsyncDisposable
{
    private readonly Process process;
    private readonly HttpClient httpClient;

    private BridgeProcess(Process process, HttpClient httpClient)
    {
        this.process = process;
        this.httpClient = httpClient;
        Client = new BridgeClient(httpClient);
    }

    public BridgeClient Client { get; }

    public static async Task<BridgeProcess> StartAsync(CancellationToken cancellationToken)
    {
        var repositoryRoot = FindRepositoryRoot();
        var token = Convert.ToHexString(RandomNumberGenerator.GetBytes(32));
        var port = GetAvailablePort();
        var python = Environment.GetEnvironmentVariable("FLING_PYTHON") ?? (OperatingSystem.IsWindows() ? "python" : "python3");

        var startInfo = new ProcessStartInfo(python)
        {
            WorkingDirectory = repositoryRoot,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        startInfo.ArgumentList.Add("-m");
        startInfo.ArgumentList.Add("bridge");
        startInfo.ArgumentList.Add("--port");
        startInfo.ArgumentList.Add(port.ToString());
        startInfo.Environment["FLING_TOKEN"] = token;
        startInfo.Environment["PYTHONPATH"] = repositoryRoot;

        var process = Process.Start(startInfo) ?? throw new InvalidOperationException("Could not start the Python bridge.");
        var httpClient = new HttpClient { BaseAddress = new Uri($"http://127.0.0.1:{port}"), Timeout = TimeSpan.FromSeconds(15) };
        httpClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);
        var bridge = new BridgeProcess(process, httpClient);

        for (var attempt = 0; attempt < 50; attempt++)
        {
            if (process.HasExited)
            {
                var error = await process.StandardError.ReadToEndAsync(cancellationToken);
                await bridge.DisposeAsync();
                throw new InvalidOperationException($"Python bridge exited: {error.Trim()}");
            }
            if (await bridge.Client.IsHealthyAsync(cancellationToken))
            {
                return bridge;
            }
            await Task.Delay(100, cancellationToken);
        }

        await bridge.DisposeAsync();
        throw new TimeoutException("Python bridge did not become ready.");
    }

    public async ValueTask DisposeAsync()
    {
        httpClient.Dispose();
        if (!process.HasExited)
        {
            process.Kill(entireProcessTree: true);
            await process.WaitForExitAsync();
        }
        process.Dispose();
    }

    private static int GetAvailablePort()
    {
        using var listener = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
        listener.Start();
        return ((System.Net.IPEndPoint)listener.LocalEndpoint).Port;
    }

    private static string FindRepositoryRoot()
    {
        var candidates = new[] { AppContext.BaseDirectory, Environment.CurrentDirectory };
        foreach (var candidate in candidates)
        {
            var directory = new DirectoryInfo(candidate);
            while (directory is not null)
            {
                if (Directory.Exists(Path.Combine(directory.FullName, "bridge")))
                {
                    return directory.FullName;
                }
                directory = directory.Parent;
            }
        }
        throw new DirectoryNotFoundException("Could not find the FlingPoc bridge directory.");
    }
}
