using Cardfile.Shared.Services;
using Microsoft.Extensions.Options;
using System.Text.Json;

namespace Cardfile.Web.Services;

/// <summary>
/// Implementación del servicio de configuración de la aplicación
/// Este servicio maneja la persistencia de configuraciones en el archivo appsettings.json
/// </summary>
public class AppSettingsService : IAppSettingsService
{
    private readonly IWebHostEnvironment _environment;
    private readonly ILogger<AppSettingsService> _logger;
    private readonly string _appSettingsPath;
    private readonly JsonSerializerOptions _jsonOptions;

    public AppSettingsService(IWebHostEnvironment environment, ILogger<AppSettingsService> logger)
    {
        _environment = environment;
        _logger = logger;
        _appSettingsPath = Path.Combine(_environment.ContentRootPath, "appsettings.json");

        _jsonOptions = new JsonSerializerOptions
        {
            WriteIndented = true,
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase
        };
    }

    /// <summary>
    /// Obtiene la configuración actual de la aplicación
    /// </summary>
    /// <returns>La configuración actual o null si no existe</returns>
    public async Task<AppSettings?> GetCurrentSettingsAsync()
    {
        try
        {
            if (!File.Exists(_appSettingsPath))
            {
                _logger.LogWarning("El archivo appsettings.json no existe en: {Path}", _appSettingsPath);
                return null;
            }

            var jsonContent = await File.ReadAllTextAsync(_appSettingsPath);
            var settings = JsonSerializer.Deserialize<Dictionary<string, object>>(jsonContent, _jsonOptions);

            if (settings == null || !settings.ContainsKey("cardfileSettings"))
            {
                return null;
            }

            var cardfileSection = JsonSerializer.Deserialize<AppSettings>(
                settings["cardfileSettings"].ToString()!, _jsonOptions);

            return cardfileSection;
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al leer la configuración desde appsettings.json");
            return null;
        }
    }

    /// <summary>
    /// Guarda la configuración de la aplicación
    /// </summary>
    /// <param name="settings">Configuración a guardar</param>
    /// <returns>Tarea asíncrona</returns>
    public async Task SaveSettingsAsync(AppSettings settings)
    {
        try
        {
            settings.LastUpdated = DateTime.UtcNow;

            // Leer configuración existente
            Dictionary<string, object> existingSettings;

            if (File.Exists(_appSettingsPath))
            {
                var jsonContent = await File.ReadAllTextAsync(_appSettingsPath);
                existingSettings = JsonSerializer.Deserialize<Dictionary<string, object>>(jsonContent, _jsonOptions) 
                                 ?? new Dictionary<string, object>();
            }
            else
            {
                existingSettings = new Dictionary<string, object>();
            }

            // Actualizar sección de Cardfile
            existingSettings["cardfileSettings"] = settings;

            // Escribir de vuelta al archivo
            var updatedJson = JsonSerializer.Serialize(existingSettings, _jsonOptions);
            await File.WriteAllTextAsync(_appSettingsPath, updatedJson);

            _logger.LogInformation("Configuración guardada exitosamente en appsettings.json");
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al guardar la configuración en appsettings.json");
            throw;
        }
    }

    /// <summary>
    /// Verifica si la aplicación ya ha sido configurada inicialmente
    /// </summary>
    /// <returns>True si está configurada, false en caso contrario</returns>
    public async Task<bool> IsConfiguredAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.IsConfigured ?? false;
    }

    /// <summary>
    /// Actualiza la configuración de base de datos
    /// </summary>
    /// <param name="databaseType">Tipo de base de datos</param>
    /// <param name="connectionString">Cadena de conexión</param>
    /// <returns>Tarea asíncrona</returns>
    public async Task UpdateDatabaseConfigurationAsync(string databaseType, string connectionString)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();

        settings.DatabaseConfiguration.DatabaseType = databaseType;
        settings.DatabaseConfiguration.ConnectionString = connectionString;
        settings.IsConfigured = true;

        await SaveSettingsAsync(settings);
    }

    /// <summary>
    /// Actualiza el idioma de la aplicación
    /// </summary>
    /// <param name="language">Código del idioma</param>
    /// <returns>Tarea asíncrona</returns>
    public async Task UpdateLanguageAsync(string language)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();
        settings.Language = language;
        await SaveSettingsAsync(settings);
    }

    /// <summary>
    /// Actualiza la información del último usuario
    /// </summary>
    /// <param name="lastUser">Información del último usuario</param>
    /// <returns>Tarea asíncrona</returns>
    public async Task UpdateLastUserAsync(LastUserInfo lastUser)
    {
        var settings = await GetCurrentSettingsAsync() ?? new AppSettings();
        settings.LastUser = lastUser;
        await SaveSettingsAsync(settings);
    }

    /// <summary>
    /// Obtiene la configuración de base de datos actual
    /// </summary>
    /// <returns>Configuración de base de datos</returns>
    public async Task<DatabaseConfiguration?> GetDatabaseConfigurationAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.DatabaseConfiguration;
    }

    /// <summary>
    /// Obtiene el idioma configurado
    /// </summary>
    /// <returns>Código del idioma</returns>
    public async Task<string> GetLanguageAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.Language ?? "es";
    }

    /// <summary>
    /// Obtiene la información del último usuario
    /// </summary>
    /// <returns>Información del último usuario</returns>
    public async Task<LastUserInfo?> GetLastUserAsync()
    {
        var settings = await GetCurrentSettingsAsync();
        return settings?.LastUser;
    }
}