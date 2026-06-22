using Cardfile.Shared.Services;
using Cardfile.Shared.Data;
using Microsoft.EntityFrameworkCore;
using Cardfile.Shared.Models;

namespace Cardfile.Services;

/// <summary>
/// AppSettingsService para el proyecto MAUI.
/// - Persistencia global: base de datos (tabla AppConfig)
/// - Persistencia por usuario (UISettings, Language): base de datos (tabla UserPreferences)
/// - Sin SQL directo: todas las operaciones de datos usan EF Core
/// - Sin serialización JSON: configuración completamente en base de datos
/// </summary>
public class AppSettingsService : IAppSettingsService
{
    private readonly ILogger<AppSettingsService> _logger;
    private readonly CardfileDbContext _db;
    private readonly IAuthService _authService;

    public AppSettingsService(
        ILogger<AppSettingsService> logger,
        CardfileDbContext db,
        IAuthService authService)
    {
        _logger = logger;
        _db = db;
        _authService = authService;
    }

    /// <summary>
    /// Obtiene la configuración global actual desde la base de datos.
    /// </summary>
    public async Task<AppSettings?> GetCurrentSettingsAsync()
    {
        try
        {
            var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
            if (appConfig == null)
            {
                return null;
            }

            return new AppSettings
            {
                IsConfigured = appConfig.IsConfigured,
                DatabaseConfiguration = new DatabaseConfiguration
                {
                    DatabaseType = appConfig.DatabaseType,
                    ConnectionString = appConfig.ConnectionString ?? "Data Source=cardfile.db",
                    AdditionalSettings = new Dictionary<string, string>()
                },
                Language = appConfig.Language ?? "es",
                LastUser = new LastUserInfo
                {
                    Username = appConfig.LastUser ?? string.Empty,
                    Email = appConfig.LastUserEmail ?? string.Empty,
                    RememberCredentials = appConfig.RememberCredentials,
                    LastLogin = appConfig.LastLoginDate ?? DateTime.UtcNow
                },
                UISettings = new UISettings
                {
                    ShowStatisticsCards = appConfig.ShowStatisticsCards,
                    CompactCards = appConfig.CompactCards,
                    ShowAttachments = appConfig.ShowAttachments,
                    ThemeMode = appConfig.ThemeMode,
                    AccentColor = appConfig.AccentColor
                },
                LastUpdated = appConfig.LastUpdated ?? DateTime.UtcNow
            };
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al obtener la configuración desde la base de datos");
            return null;
        }
    }

    /// <summary>
    /// Guarda configuración global en la base de datos.
    /// </summary>
    public async Task SaveSettingsAsync(AppSettings settings)
    {
        try
        {
            var appConfig = await _db.AppConfigs.FirstOrDefaultAsync();
            if (appConfig == null)
            {
                appConfig = new AppConfig { Id = Guid.NewGuid() };
                _db.AppConfigs.Add(appConfig);
            }

            appConfig.IsConfigured = settings.IsConfigured;
            appConfig.DatabaseType = settings.DatabaseConfiguration.DatabaseType;
            appConfig.ConnectionString = settings.DatabaseConfiguration.ConnectionString;
            appConfig.Language = settings.Language;
            
            if (settings.LastUser != null)
            {
                appConfig.LastUser = settings.LastUser.Username;
                appConfig.LastUserEmail = settings.LastUser.Email;
                appConfig.RememberCredentials = settings.LastUser.RememberCredentials;
                appConfig.LastLoginDate = settings.LastUser.LastLogin;
            }

            // Configuraciones globales de UI
            appConfig.ShowStatisticsCards = settings.UISettings.ShowStatisticsCards;
            appConfig.CompactCards = settings.UISettings.CompactCards;
            appConfig.ShowAttachments = settings.UISettings.ShowAttachments;
            appConfig.ThemeMode = settings.UISettings.ThemeMode;
            appConfig.AccentColor = settings.UISettings.AccentColor;
            
            appConfig.LastUpdated = DateTime.UtcNow;

            await _db.SaveChangesAsync();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al guardar la configuración en la base de datos");
            throw;
        }
    }

    /// <summary>
    /// Verifica si la aplicación ya ha sido configurada.
    /// </summary>
    public async Task<bool> IsConfiguredAsync()
    {
        try
        {
            var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
            return appConfig?.IsConfigured ?? false;
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al verificar si la aplicación está configurada");
            return false;
        }
    }

    /// <summary>
    /// Actualiza la configuración de base de datos.
    /// </summary>
    public async Task UpdateDatabaseConfigurationAsync(string databaseType, string connectionString)
    {
        try
        {
            var appConfig = await _db.AppConfigs.FirstOrDefaultAsync();
            if (appConfig == null)
            {
                appConfig = new AppConfig { Id = Guid.NewGuid() };
                _db.AppConfigs.Add(appConfig);
            }

            appConfig.DatabaseType = databaseType;
            appConfig.ConnectionString = connectionString;
            appConfig.LastUpdated = DateTime.UtcNow;

            await _db.SaveChangesAsync();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al actualizar la configuración de base de datos");
            throw;
        }
    }

    /// <summary>
    /// Actualiza el idioma de la aplicación.
    /// </summary>
    public async Task UpdateLanguageAsync(string language)
    {
        var currentUser = await _authService.GetCurrentUserAsync();
        if (currentUser != null)
        {
            // Usuario autenticado: guardar en UserPreferences
            var prefs = await _db.UserPreferences.FirstOrDefaultAsync(p => p.UserId == currentUser.Id);
            if (prefs == null)
            {
                prefs = new UserPreferences { Id = Guid.NewGuid(), UserId = currentUser.Id };
                _db.UserPreferences.Add(prefs);
            }

            prefs.Language = language;
            prefs.LastUpdated = DateTime.UtcNow;
        }
        else
        {
            // Usuario no autenticado: guardar en AppConfig global
            var appConfig = await _db.AppConfigs.FirstOrDefaultAsync();
            if (appConfig == null)
            {
                appConfig = new AppConfig { Id = Guid.NewGuid() };
                _db.AppConfigs.Add(appConfig);
            }

            appConfig.Language = language;
            appConfig.LastUpdated = DateTime.UtcNow;
        }

        await _db.SaveChangesAsync();
    }

    /// <summary>
    /// Obtiene el idioma actual.
    /// </summary>
    public async Task<string> GetLanguageAsync()
    {
        var currentUser = await _authService.GetCurrentUserAsync();
        if (currentUser != null)
        {
            // Usuario autenticado: obtener de UserPreferences
            var prefs = await _db.UserPreferences.AsNoTracking().FirstOrDefaultAsync(p => p.UserId == currentUser.Id);
            if (prefs?.Language != null)
            {
                return prefs.Language;
            }
        }

        // Fallback: obtener de AppConfig global
        var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
        return appConfig?.Language ?? "es";
    }

    /// <summary>
    /// Actualiza la información del último usuario.
    /// </summary>
    public async Task UpdateLastUserAsync(LastUserInfo lastUser)
    {
        try
        {
            var appConfig = await _db.AppConfigs.FirstOrDefaultAsync();
            if (appConfig == null)
            {
                appConfig = new AppConfig { Id = Guid.NewGuid() };
                _db.AppConfigs.Add(appConfig);
            }

            appConfig.LastUser = lastUser.Username;
            appConfig.LastUserEmail = lastUser.Email;
            appConfig.RememberCredentials = lastUser.RememberCredentials;
            appConfig.LastLoginDate = lastUser.LastLogin;
            appConfig.LastUpdated = DateTime.UtcNow;

            await _db.SaveChangesAsync();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al actualizar la información del último usuario");
            throw;
        }
    }

    /// <summary>
    /// Obtiene la información del último usuario.
    /// </summary>
    public async Task<LastUserInfo?> GetLastUserAsync()
    {
        try
        {
            var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
            if (appConfig == null)
            {
                return null;
            }

            return new LastUserInfo
            {
                Username = appConfig.LastUser ?? string.Empty,
                Email = appConfig.LastUserEmail ?? string.Empty,
                RememberCredentials = appConfig.RememberCredentials,
                LastLogin = appConfig.LastLoginDate ?? DateTime.UtcNow
            };
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al obtener la información del último usuario");
            return null;
        }
    }

    /// <summary>
    /// Obtiene las configuraciones de UI desde DB por usuario; si no hay usuario, obtiene configuración global.
    /// </summary>
    public async Task<UISettings> GetUISettingsAsync()
    {
        var currentUser = await _authService.GetCurrentUserAsync();
        if (currentUser != null)
        {
            // Usuario autenticado: obtener de UserPreferences
            var prefs = await _db.UserPreferences.AsNoTracking().FirstOrDefaultAsync(p => p.UserId == currentUser.Id);
            if (prefs != null)
            {
                return new UISettings
                {
                    ShowStatisticsCards = prefs.ShowStatisticsCards,
                    CompactCards = prefs.CompactCards,
                    ShowAttachments = prefs.ShowAttachments,
                    ThemeMode = prefs.ThemeMode,
                    AccentColor = prefs.AccentColor
                };
            }
        }

        // Fallback: obtener configuración global de AppConfig
        var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
        if (appConfig != null)
        {
            return new UISettings
            {
                ShowStatisticsCards = appConfig.ShowStatisticsCards,
                CompactCards = appConfig.CompactCards,
                ShowAttachments = appConfig.ShowAttachments,
                ThemeMode = appConfig.ThemeMode,
                AccentColor = appConfig.AccentColor
            };
        }

        // Valores por defecto si no hay configuración
        return new UISettings();
    }

    /// <summary>
    /// Actualiza y persiste configuraciones de UI por usuario en DB.
    /// Si no hay sesión de usuario, guarda en configuración global.
    /// </summary>
    public async Task UpdateUISettingsAsync(UISettings settingsToSave)
    {
        var currentUser = await _authService.GetCurrentUserAsync();
        if (currentUser != null)
        {
            // Usuario autenticado: guardar en UserPreferences
            var prefs = await _db.UserPreferences.FirstOrDefaultAsync(p => p.UserId == currentUser.Id);
            if (prefs == null)
            {
                prefs = new UserPreferences { Id = Guid.NewGuid(), UserId = currentUser.Id };
                _db.UserPreferences.Add(prefs);
            }

            prefs.ShowStatisticsCards = settingsToSave?.ShowStatisticsCards ?? true;
            prefs.CompactCards = settingsToSave?.CompactCards ?? false;
            prefs.ShowAttachments = settingsToSave?.ShowAttachments ?? true;
            prefs.ThemeMode = settingsToSave?.ThemeMode ?? "system";
            prefs.AccentColor = settingsToSave?.AccentColor ?? "#0063B1";
            prefs.LastUpdated = DateTime.UtcNow;
        }
        else
        {
            // Usuario no autenticado: guardar en AppConfig global
            var appConfig = await _db.AppConfigs.FirstOrDefaultAsync();
            if (appConfig == null)
            {
                appConfig = new AppConfig { Id = Guid.NewGuid() };
                _db.AppConfigs.Add(appConfig);
            }

            appConfig.ShowStatisticsCards = settingsToSave?.ShowStatisticsCards ?? true;
            appConfig.CompactCards = settingsToSave?.CompactCards ?? false;
            appConfig.ShowAttachments = settingsToSave?.ShowAttachments ?? true;
            appConfig.ThemeMode = settingsToSave?.ThemeMode ?? "system";
            appConfig.AccentColor = settingsToSave?.AccentColor ?? "#0063B1";
            appConfig.LastUpdated = DateTime.UtcNow;
        }

        await _db.SaveChangesAsync();
    }

    /// <summary>
    /// Obtiene la configuración de base de datos actual.
    /// </summary>
    public async Task<DatabaseConfiguration?> GetDatabaseConfigurationAsync()
    {
        try
        {
            var appConfig = await _db.AppConfigs.AsNoTracking().FirstOrDefaultAsync();
            if (appConfig == null)
            {
                return null;
            }

            return new DatabaseConfiguration
            {
                DatabaseType = appConfig.DatabaseType,
                ConnectionString = appConfig.ConnectionString ?? "Data Source=cardfile.db",
                AdditionalSettings = new Dictionary<string, string>()
            };
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al obtener la configuración de base de datos");
            return null;
        }
    }
}