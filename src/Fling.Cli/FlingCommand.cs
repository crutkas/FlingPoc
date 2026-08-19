using System.Net;

namespace Fling.Cli;

public static class FlingCommand
{
    public static async Task<int> RunAsync(string[] args)
    {
        if (args.Length == 0 || args[0] is "-h" or "--help" or "help")
        {
            PrintHelp();
            return 0;
        }

        using var cancellation = new CancellationTokenSource();
        Console.CancelKeyPress += (_, eventArgs) =>
        {
            eventArgs.Cancel = true;
            cancellation.Cancel();
        };

        try
        {
            await using var bridge = await BridgeProcess.StartAsync(cancellation.Token);
            var client = bridge.Client;
            switch (args[0])
            {
                case "devices":
                    foreach (var device in await client.GetDevicesAsync(cancellation.Token))
                    {
                        Console.WriteLine($"{device.Id}\t{device.Name}\t{device.Address}\t{string.Join(',', device.Protocols)}");
                    }
                    break;
                case "pair" when args.Length == 2:
                    var pairing = await client.StartPairingAsync(args[1], cancellation.Token);
                    Console.Write("PIN shown on Apple TV (or requested by it): ");
                    var pin = Console.ReadLine() ?? "";
                    await client.FinishPairingAsync(pairing.SessionId, pin, cancellation.Token);
                    Console.WriteLine("Paired. pyatv saved the credentials for the current user.");
                    break;
                case "cast-url" when args.Length == 3:
                    await client.PlayUrlAsync(args[1], args[2], cancellation.Token);
                    await WaitForStopAsync("Casting. Press Ctrl+C to stop.", cancellation.Token);
                    break;
                case "cast-file" when args.Length == 3:
                    var file = Path.GetFullPath(args[2]);
                    if (!File.Exists(file))
                    {
                        throw new FileNotFoundException("Media file not found.", file);
                    }
                    await client.PlayFileAsync(args[1], file, cancellation.Token);
                    await WaitForStopAsync("Serving media. Press Ctrl+C to stop.", cancellation.Token);
                    break;
                case "mirror" when args.Length == 2:
                    await client.StartMirrorAsync(args[1], cancellation.Token);
                    await WaitForStopAsync("Mirroring. Press Ctrl+C to stop.", cancellation.Token);
                    break;
                case "stop" when args.Length == 2:
                    await client.StopAsync(args[1], cancellation.Token);
                    Console.WriteLine("Stop requested.");
                    break;
                case "status" when args.Length == 2:
                    var status = await client.GetStatusAsync(args[1], cancellation.Token);
                    Console.WriteLine($"{status.State}\t{status.Title ?? "(untitled)"}\t{status.Position:0}/{status.Duration:0}s");
                    break;
                default:
                    PrintHelp();
                    return 2;
            }
            return 0;
        }
        catch (OperationCanceledException)
        {
            return 130;
        }
        catch (BridgeException exception)
        {
            Console.Error.WriteLine($"Bridge error ({(int)exception.StatusCode}): {exception.Message}");
            return 1;
        }
        catch (Exception exception)
        {
            Console.Error.WriteLine(exception.Message);
            return 1;
        }
    }

    private static async Task WaitForStopAsync(string message, CancellationToken cancellationToken)
    {
        Console.WriteLine(message);
        try
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            // Normal interactive shutdown.
        }
    }

    private static void PrintHelp()
    {
        Console.WriteLine("""
            FlingPoc - AirPlay video casting and desktop streaming

              devices
              pair <device-id>
              cast-url <device-id> <http(s)-url>
              cast-file <device-id> <file>
              mirror <device-id>
              status <device-id>
              stop <device-id>

            FFmpeg must be on PATH for mirror. Set FLING_PYTHON to select Python.
            """);
    }
}
