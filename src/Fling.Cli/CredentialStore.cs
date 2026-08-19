using System.Text.Json;

namespace Fling.Cli;

public static class CredentialStore
{
    private static readonly string StorePath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "FlingPoc",
        "credentials.json");

    public static Dictionary<string, string> Load()
    {
        if (!File.Exists(StorePath))
        {
            return [];
        }
        return JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(StorePath)) ?? [];
    }

    public static void Save(string deviceId, string credentials)
    {
        var values = Load();
        values[deviceId] = credentials;
        Directory.CreateDirectory(Path.GetDirectoryName(StorePath)!);
        File.WriteAllText(StorePath, JsonSerializer.Serialize(values, new JsonSerializerOptions { WriteIndented = true }));
    }
}
