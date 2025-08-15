using Cardfile.Shared.Models;
using Cardfile.Shared.Services;
using Microsoft.Extensions.Logging;
using Microsoft.Maui.Storage;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Cardfile.Services;

/// <summary>
/// AppSettingsService implementation for .NET MAUI platform.
/// Persists application configuration into a local appsettings.json file under the app data directory.
/// This mirrors the behavior of the web project's AppSettingsService but uses MAUI's FileSystem APIs.
/// </summary>
public class AppSettingsService : IAppSettingsService
{
    private readonly string _settingsFilePath;
    private readonly JsonSerializerOptions _jsonOptions;
    private readonly ILogger<AppSettingsService> _logger;

    public AppSettingsService(ILogger<AppSettingsService> logger)
    {
        _logger = logger;
        var appDataDir = FileSystem.AppDataDirectory;
        Directory.CreateDirectory(appDataDir);
        _settingsFilePath = Path.Combine(appDataDir, "appsettings.json");

        _jsonOptions = new JsonSerializerOptions
        {
            WriteIndented = true,
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase
        };
    }

    /// <inheritdoc />
    public async Task<AppSettings?> GetCurrentSettingsAsync()
    {
        try
        {
            if (!File.Exists(_settingsFilePath))
            {
                return null;
            }

            var jsonContent = await File.ReadAllTextAsync(_settingsFilePath);
            var root = JsonNode.Parse(jsonContent) as JsonObject;
            if (root is null)
            {
                return null;
            }

            var settingsNode = root["cardfileSettings"];
            if (settingsNode is null)
            {
                return null;
            }

            return settingsNode.Deserialize<AppSettings>(_jsonOptions);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error reading appsettings.json in MAUI");
            return null;
        }
    }

    /// <inheritdoc />
    public async Task SaveSettingsAsync(AppSettings settings)
    {
        try
        {
            settings.LastUpdated = DateTime.UtcNow;

            JsonObject root;
            if (File.Exists(_settingsFilePath))
            {
                var json = await File.ReadAllTextAsync(_settingsFilePath);
                root = (JsonNode.Parse(json) as JsonObject) ?? new JsonObject();
            }
            else
            {
                root = new JsonObject();
            }

            root["cardfileSettings"] = JsonSerializer.SerializeToNode(settings, _jsonOptions);

            var updated = root.ToJsonString(_jsonOptions);
            await File.WriteAllTextAsync(_settingsFilePath, updated);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error saving appsettings.json in MAUI");
            throw;
        }
    }

    /// <inheritdoc />
    public async Task<bool> IsConfiguredAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.IsConfigured ?? false;
    }

    /// <inheritdoc />
    public async Task UpdateDatabaseConfigurationAsync(string databaseType, string connectionString)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();
        settings.DatabaseConfiguration.DatabaseType = databaseType;
        settings.DatabaseConfiguration.ConnectionString = connectionString;
        settings.IsConfigured = true;
        await SaveSettingsAsync(settings);
    }

    /// <inheritdoc />
    public async Task UpdateLanguageAsync(string language)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();
        settings.Language = language;
        await SaveSettingsAsync(settings);
    }

    /// <inheritdoc />
    public async Task UpdateLastUserAsync(LastUserInfo lastUser)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();
        settings.LastUser = lastUser;
        await SaveSettingsAsync(settings);
    }

    /// <inheritdoc />
    public async Task<DatabaseConfiguration?> GetDatabaseConfigurationAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.DatabaseConfiguration;
    }

    /// <inheritdoc />
    public async Task<string> GetLanguageAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.Language ?? "es";
    }

    /// <inheritdoc />
    public async Task<LastUserInfo?> GetLastUserAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.LastUser;
    }
}