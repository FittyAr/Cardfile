using Cardfile.Shared.Services;
using Cardfile.Shared.Data;
using System.Security.Claims;
using Microsoft.EntityFrameworkCore;
using Microsoft.AspNetCore.Http;
using Cardfile.Shared.Models;

namespace Cardfile.Web.Services;

/// <summary>
/// AppSettingsService para el proyecto Web.
/// - Persistencia global: base de datos (tabla AppConfig)
/// - Persistencia por usuario (UISettings, Language): base de datos (tabla UserPreferences)
/// - Sin SQL directo: todas las operaciones de datos usan EF Core
/// - Sin serialización JSON: configuración completamente en base de datos
/// </summary>
public class AppSettingsService : IAppSettingsService
{
    private readonly ILogger<AppSettingsService> _logger;
    private readonly CardfileDbContext _db;
    private readonly IHttpContextAccessor _httpContextAccessor;

    public AppSettingsService(
        ILogger<AppSettingsService> logger,
        CardfileDbContext db,
        IHttpContextAccessor httpContextAccessor)
    {
        _logger = logger;
        _db = db;
        _httpContextAccessor = httpContextAccessor;
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
        var userId = GetCurrentUserId();
        if (userId is Guid uid)
        {
            // Usuario autenticado: guardar en UserPreferences
            var prefs = await _db.UserPreferences.FirstOrDefaultAsync(p => p.UserId == uid);
            if (prefs == null)
            {
                prefs = new UserPreferences { Id = Guid.NewGuid(), UserId = uid };
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
        var userId = GetCurrentUserId();
        if (userId is Guid uid)
        {
            // Usuario autenticado: obtener de UserPreferences
            var prefs = await _db.UserPreferences.AsNoTracking().FirstOrDefaultAsync(p => p.UserId == uid);
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
        try
        {
            var userId = GetCurrentUserId();
            _logger.LogInformation("GetUISettingsAsync - UserId: {UserId}", userId);

            if (userId is Guid uid)
            {
                // Usuario autenticado: obtener de UserPreferences
                var prefs = await _db.UserPreferences.AsNoTracking().FirstOrDefaultAsync(p => p.UserId == uid);
                if (prefs != null)
                {
                    _logger.LogInformation("Configuración encontrada en UserPreferences: {@Settings}", new { prefs.ShowStatisticsCards, prefs.CompactCards, prefs.ShowAttachments, prefs.ThemeMode, prefs.AccentColor });
                    return new UISettings
                    {
                        ShowStatisticsCards = prefs.ShowStatisticsCards,
                        CompactCards = prefs.CompactCards,
                        ShowAttachments = prefs.ShowAttachments,
                        ThemeMode = prefs.ThemeMode,
                        AccentColor = prefs.AccentColor
                    };
                }
                else
                {
                    _logger.LogInformation("No se encontraron UserPreferences para usuario {UserId}", uid);
                }
            }

            // Fallback: obtener configuración global de AppConfig
            var appConfig = await _db.AppConfigs
            .AsNoTracking()
            .OrderByDescending(a => a.LastUpdated ?? DateTime.MinValue)
            .ThenByDescending(a => a.Id)
            .FirstOrDefaultAsync();
             if (appConfig != null)
             {
                 _logger.LogInformation("Configuración encontrada en AppConfig: {@Settings}", new { appConfig.ShowStatisticsCards, appConfig.CompactCards, appConfig.ShowAttachments, appConfig.ThemeMode, appConfig.AccentColor });
                 return new UISettings
                 {
                     ShowStatisticsCards = appConfig.ShowStatisticsCards,
                     CompactCards = appConfig.CompactCards,
                     ShowAttachments = appConfig.ShowAttachments,
                     ThemeMode = appConfig.ThemeMode,
                     AccentColor = appConfig.AccentColor
                 };
             }
             else
             {
                 _logger.LogInformation("No se encontró AppConfig, usando valores por defecto");
             }

            // Valores por defecto si no hay configuración
            var defaultSettings = new UISettings();
            _logger.LogInformation("Devolviendo configuración por defecto: {@Settings}", defaultSettings);
            return defaultSettings;
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al obtener configuraciones de UI");
            throw;
        }
    }

    /// <summary>
    /// Actualiza y persiste configuraciones de UI por usuario en DB.
    /// Si no hay sesión de usuario, guarda en configuración global.
    /// </summary>
    public async Task UpdateUISettingsAsync(UISettings settingsToSave)
    {
        Console.WriteLine($"*** UpdateUISettingsAsync LLAMADO ***");
        Console.WriteLine($"*** ShowStatisticsCards: {settingsToSave?.ShowStatisticsCards} ***");
        Console.WriteLine($"*** CompactCards: {settingsToSave?.CompactCards} ***");
        Console.WriteLine($"*** ShowAttachments: {settingsToSave?.ShowAttachments} ***");
        Console.WriteLine($"*** ThemeMode: {settingsToSave?.ThemeMode} ***");
        Console.WriteLine($"*** AccentColor: {settingsToSave?.AccentColor} ***");

        try
        {
            var userId = GetCurrentUserId();
            _logger.LogInformation("UpdateUISettingsAsync - UserId: {UserId}", userId);

            if (userId is Guid uid)
            {
                // Usuario autenticado: obtener valores actuales de UserPreferences
                var prefs = await _db.UserPreferences.FirstOrDefaultAsync(p => p.UserId == uid);
                if (prefs == null)
                {
                    prefs = new UserPreferences { Id = Guid.NewGuid(), UserId = uid };
                    _db.UserPreferences.Add(prefs);
                    _logger.LogInformation("Creando nuevas UserPreferences para usuario {UserId}", uid);
                }
                else
                {
                    _logger.LogInformation("Valores actuales en DB - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}", 
                        prefs.ShowStatisticsCards, prefs.CompactCards, prefs.ShowAttachments, prefs.ThemeMode, prefs.AccentColor);
                }

                _logger.LogInformation("Valores nuevos - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}", 
                    settingsToSave?.ShowStatisticsCards, settingsToSave?.CompactCards, settingsToSave?.ShowAttachments, settingsToSave?.ThemeMode, settingsToSave?.AccentColor);

                // Verificar el estado de tracking antes de los cambios
                var entityEntry = _db.Entry(prefs);
                _logger.LogInformation("Estado de tracking antes: {State}", entityEntry.State);

                prefs.ShowStatisticsCards = settingsToSave?.ShowStatisticsCards ?? true;
                prefs.CompactCards = settingsToSave?.CompactCards ?? false;
                prefs.ShowAttachments = settingsToSave?.ShowAttachments ?? true;
                prefs.ThemeMode = settingsToSave?.ThemeMode ?? "system";
                prefs.AccentColor = settingsToSave?.AccentColor ?? "#0063B1";
                prefs.LastUpdated = DateTime.UtcNow;

                // Verificar el estado de tracking después de los cambios
                _logger.LogInformation("Estado de tracking después: {State}", entityEntry.State);

                // Forzar la detección de cambios
                _db.ChangeTracker.DetectChanges();
                _logger.LogInformation("Estado de tracking después de DetectChanges: {State}", entityEntry.State);

                // Log de propiedades modificadas
                var modifiedProperties = entityEntry.Properties.Where(p => p.IsModified).Select(p => p.Metadata.Name).ToList();
                _logger.LogInformation("Propiedades modificadas: {Properties}", string.Join(", ", modifiedProperties));
            }
            else
            {
                // Usuario no autenticado: obtener o garantizar AppConfig único
                var appConfig = await EnsureSingletonAppConfigAsync();
                _logger.LogInformation("Valores actuales en DB - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}", 
                    appConfig.ShowStatisticsCards, appConfig.CompactCards, appConfig.ShowAttachments, appConfig.ThemeMode, appConfig.AccentColor);

                _logger.LogInformation("Valores nuevos - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}", 
                    settingsToSave?.ShowStatisticsCards, settingsToSave?.CompactCards, settingsToSave?.ShowAttachments, settingsToSave?.ThemeMode, settingsToSave?.AccentColor);

                // Verificar el estado de tracking antes de los cambios
                var entityEntry = _db.Entry(appConfig);
                _logger.LogInformation("Estado de tracking antes: {State}", entityEntry.State);

                appConfig.ShowStatisticsCards = settingsToSave?.ShowStatisticsCards ?? true;
                appConfig.CompactCards = settingsToSave?.CompactCards ?? false;
                appConfig.ShowAttachments = settingsToSave?.ShowAttachments ?? true;
                appConfig.ThemeMode = settingsToSave?.ThemeMode ?? "system";
                appConfig.AccentColor = settingsToSave?.AccentColor ?? "#0063B1";
                appConfig.LastUpdated = DateTime.UtcNow;
                
                // Verificar el estado de tracking después de los cambios
                _logger.LogInformation("Estado de tracking después: {State}", entityEntry.State);

                // Forzar la detección de cambios
                _db.ChangeTracker.DetectChanges();
                _logger.LogInformation("Estado de tracking después de DetectChanges: {State}", entityEntry.State);

                // Log de propiedades modificadas
                var modifiedProperties = entityEntry.Properties.Where(p => p.IsModified).Select(p => p.Metadata.Name).ToList();
                _logger.LogInformation("Propiedades modificadas: {Properties}", string.Join(", ", modifiedProperties));
            }

            var changes = await _db.SaveChangesAsync();
            _logger.LogInformation("SaveChangesAsync completado. Cambios guardados: {Changes}", changes);

            // Releer desde DB para confirmar persistencia inmediata
            if (userId is Guid confirmUid)
            {
                var persisted = await _db.UserPreferences.AsNoTracking().FirstOrDefaultAsync(p => p.UserId == confirmUid);
                if (persisted != null)
                {
                    _logger.LogInformation("Valores persistidos en UserPreferences - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}",
                        persisted.ShowStatisticsCards, persisted.CompactCards, persisted.ShowAttachments, persisted.ThemeMode, persisted.AccentColor);
                }
            }
            else
            {
                var persisted = await _db.AppConfigs.AsNoTracking()
                    .OrderByDescending(a => a.LastUpdated ?? DateTime.MinValue)
                    .ThenByDescending(a => a.Id)
                    .FirstOrDefaultAsync();
                if (persisted != null)
                {
                    _logger.LogInformation("Valores persistidos en AppConfig - ShowStatisticsCards: {ShowStatisticsCards}, CompactCards: {CompactCards}, ShowAttachments: {ShowAttachments}, ThemeMode: {ThemeMode}, AccentColor: {AccentColor}",
                        persisted.ShowStatisticsCards, persisted.CompactCards, persisted.ShowAttachments, persisted.ThemeMode, persisted.AccentColor);
                }
            }
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error al actualizar configuraciones de UI");
            throw;
        }
    }

    /// <summary>
    /// Garantiza que exista un único registro AppConfig en la base de datos.
    /// Si hay múltiples, conserva el más reciente y elimina el resto.
    /// Devuelve la entidad AppConfig seleccionada, adjunta al contexto para edición.
    /// </summary>
    private async Task<AppConfig> EnsureSingletonAppConfigAsync()
    {
        var list = await _db.AppConfigs
            .OrderByDescending(a => a.LastUpdated ?? DateTime.MinValue)
            .ThenByDescending(a => a.Id)
            .ToListAsync();

        if (list.Count == 0)
        {
            var created = new AppConfig { Id = Guid.NewGuid(), LastUpdated = DateTime.UtcNow };
            _db.AppConfigs.Add(created);
            _logger.LogWarning("No existía AppConfig. Creado uno nuevo con Id={Id}", created.Id);
            await _db.SaveChangesAsync();
            return created;
        }

        var keep = list[0];
        var duplicates = list.Skip(1).ToList();
        if (duplicates.Count > 0)
        {
            _db.AppConfigs.RemoveRange(duplicates);
            _logger.LogWarning("Se encontraron {Count} AppConfig duplicados. Se conservará Id={Id} y se eliminarán los demás.", duplicates.Count, keep.Id);
            await _db.SaveChangesAsync();
        }

        // Asegurar que la entidad seleccionada esté adjunta para edición si es necesario
        if (_db.Entry(keep).State == EntityState.Detached)
        {
            _db.Attach(keep);
        }

        return keep;
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

    /// <summary>
    /// Obtiene el ID del usuario autenticado actual.
    /// </summary>
    private Guid? GetCurrentUserId()
    {
        var httpContext = _httpContextAccessor.HttpContext;
        if (httpContext?.User?.Identity?.IsAuthenticated == true)
        {
            var userIdClaim = httpContext.User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            if (Guid.TryParse(userIdClaim, out var userId))
            {
                return userId;
            }
        }
        return null;
    }
}