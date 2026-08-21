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
        var launch = CreateLaunch(repositoryRoot, port);
        var startInfo = launch.StartInfo;
        startInfo.Environment["FLING_TOKEN"] = token;

        var process = Process.Start(startInfo) ?? throw new InvalidOperationException($"Could not start the {launch.Name}.");
        var httpClient = new HttpClient { BaseAddress = new Uri($"http://127.0.0.1:{port}"), Timeout = TimeSpan.FromSeconds(15) };
        httpClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);
        var bridge = new BridgeProcess(process, httpClient);

        try
        {
            for (var attempt = 0; attempt < 50; attempt++)
            {
                if (process.HasExited)
                {
                    var error = await process.StandardError.ReadToEndAsync(cancellationToken);
                    throw new InvalidOperationException($"{launch.Name} exited: {error.Trim()}");
                }
                if (await bridge.Client.IsHealthyAsync(cancellationToken))
                {
                    return bridge;
                }
                await Task.Delay(100, cancellationToken);
            }
            throw new TimeoutException($"{launch.Name} did not become ready.");
        }
        catch
        {
            await bridge.DisposeAsync();
            throw;
        }
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

    private static BridgeLaunch CreateLaunch(string repositoryRoot, int port)
    {
        var implementation = (Environment.GetEnvironmentVariable("FLING_BRIDGE") ?? "python").Trim().ToLowerInvariant();
        return implementation switch
        {
            "python" => CreatePythonLaunch(repositoryRoot, port),
            "rust" => CreateRustLaunch(repositoryRoot, port),
            _ => throw new InvalidOperationException("FLING_BRIDGE must be either 'python' or 'rust'."),
        };
    }

    private static BridgeLaunch CreatePythonLaunch(string repositoryRoot, int port)
    {
        var python = Environment.GetEnvironmentVariable("FLING_PYTHON") ?? (OperatingSystem.IsWindows() ? "python" : "python3");
        var startInfo = NewStartInfo(python, repositoryRoot);
        startInfo.ArgumentList.Add("-m");
        startInfo.ArgumentList.Add("bridge");
        startInfo.ArgumentList.Add("--port");
        startInfo.ArgumentList.Add(port.ToString());
        startInfo.Environment["PYTHONPATH"] = repositoryRoot;
        return new(startInfo, "Python bridge");
    }

    private static BridgeLaunch CreateRustLaunch(string repositoryRoot, int port)
    {
        var configuredSidecar = Environment.GetEnvironmentVariable("FLING_RUST_SIDECAR");
        var executable = configuredSidecar ?? FindBuiltRustSidecar(repositoryRoot);
        var startInfo = NewStartInfo(executable, repositoryRoot);
        startInfo.ArgumentList.Add("--port");
        startInfo.ArgumentList.Add(port.ToString());
        return new(startInfo, "Rust bridge");
    }

    private static string FindBuiltRustSidecar(string repositoryRoot)
    {
        var executableName = OperatingSystem.IsWindows() ? "fling-airplay-sidecar.exe" : "fling-airplay-sidecar";
        var targetRoot = Path.Combine(repositoryRoot, "run", "target");
        var candidates = new[]
        {
            Path.Combine(targetRoot, "release", executableName),
            Path.Combine(targetRoot, "debug", executableName),
        };
        return candidates.FirstOrDefault(File.Exists)
            ?? throw new FileNotFoundException(
                "Rust sidecar was not found. Run 'cargo build --release --manifest-path run/Cargo.toml' or set FLING_RUST_SIDECAR.");
    }

    private static ProcessStartInfo NewStartInfo(string executable, string repositoryRoot) =>
        new(executable)
        {
            WorkingDirectory = repositoryRoot,
            RedirectStandardError = true,
            UseShellExecute = false,
        };

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

    private sealed record BridgeLaunch(ProcessStartInfo StartInfo, string Name);
}
