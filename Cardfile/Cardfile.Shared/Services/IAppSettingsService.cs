using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services;

/// <summary>
/// Interfaz para el servicio de gestión de configuración de la aplicación
/// Este servicio maneja la persistencia de configuraciones en el archivo appsettings.json
/// </summary>
public interface IAppSettingsService
{
    /// <summary>
    /// Obtiene la configuración actual de la aplicación
    /// </summary>
    /// <returns>La configuración actual o null si no existe</returns>
    Task<AppSettings?> GetCurrentSettingsAsync();

    /// <summary>
    /// Guarda la configuración de la aplicación
    /// </summary>
    /// <param name="settings">Configuración a guardar</param>
    /// <returns>Tarea asíncrona</returns>
    Task SaveSettingsAsync(AppSettings settings);

    /// <summary>
    /// Verifica si la aplicación ya ha sido configurada inicialmente
    /// </summary>
    /// <returns>True si está configurada, false en caso contrario</returns>
    Task<bool> IsConfiguredAsync();

    /// <summary>
    /// Actualiza la configuración de base de datos
    /// </summary>
    /// <param name="databaseType">Tipo de base de datos</param>
    /// <param name="connectionString">Cadena de conexión</param>
    /// <returns>Tarea asíncrona</returns>
    Task UpdateDatabaseConfigurationAsync(string databaseType, string connectionString);

    /// <summary>
    /// Actualiza el idioma de la aplicación
    /// </summary>
    /// <param name="language">Código del idioma</param>
    /// <returns>Tarea asíncrona</returns>
    Task UpdateLanguageAsync(string language);

    /// <summary>
    /// Actualiza la información del último usuario
    /// </summary>
    /// <param name="lastUser">Información del último usuario</param>
    /// <returns>Tarea asíncrona</returns>
    Task UpdateLastUserAsync(LastUserInfo lastUser);

    /// <summary>
    /// Obtiene la configuración de base de datos actual
    /// </summary>
    /// <returns>Configuración de base de datos</returns>
    Task<DatabaseConfiguration?> GetDatabaseConfigurationAsync();

    /// <summary>
    /// Obtiene el idioma configurado
    /// </summary>
    /// <returns>Código del idioma</returns>
    Task<string> GetLanguageAsync();

    /// <summary>
    /// Obtiene la información del último usuario
    /// </summary>
    /// <returns>Información del último usuario</returns>
    Task<LastUserInfo?> GetLastUserAsync();

    /// <summary>
    /// Obtiene las configuraciones de interfaz de usuario (UI)
    /// </summary>
    /// <returns>Configuraciones de UI actuales</returns>
    Task<UISettings> GetUISettingsAsync();

    /// <summary>
    /// Actualiza las configuraciones de interfaz de usuario (UI)
    /// </summary>
    /// <param name="settings">Nuevas configuraciones de UI</param>
    Task UpdateUISettingsAsync(UISettings settings);
}

/// <summary>
/// Modelo para la configuración de la aplicación almacenada en appsettings.json
/// </summary>
public class AppSettings
{
    /// <summary>
    /// Indica si la aplicación ha sido configurada inicialmente
    /// </summary>
    public bool IsConfigured { get; set; }

    /// <summary>
    /// Configuración de base de datos
    /// </summary>
    public DatabaseConfiguration DatabaseConfiguration { get; set; } = new();

    /// <summary>
    /// Idioma de la aplicación
    /// </summary>
    public string Language { get; set; } = "es";

    /// <summary>
    /// Información del último usuario que utilizó la aplicación
    /// </summary>
    public LastUserInfo? LastUser { get; set; }

    /// <summary>
    /// Configuraciones de la interfaz de usuario
    /// </summary>
    public UISettings UISettings { get; set; } = new();

    /// <summary>
    /// Fecha de última actualización de la configuración
    /// </summary>
    public DateTime LastUpdated { get; set; } = DateTime.UtcNow;
}

/// <summary>
/// Configuración de base de datos
/// </summary>
public class DatabaseConfiguration
{
    /// <summary>
    /// Tipo de base de datos (SQLite, MySQL, MariaDB, MsSQL)
    /// </summary>
    public string DatabaseType { get; set; } = "SQLite";

    /// <summary>
    /// Cadena de conexión a la base de datos
    /// </summary>
    public string ConnectionString { get; set; } = "Data Source=cardfile.db";

    /// <summary>
    /// Configuraciones adicionales específicas del proveedor
    /// </summary>
    public Dictionary<string, string> AdditionalSettings { get; set; } = new();
}

/// <summary>
/// Información del último usuario
/// </summary>
public class LastUserInfo
{
    /// <summary>
    /// Nombre de usuario del último usuario
    /// </summary>
    public string Username { get; set; } = string.Empty;

    /// <summary>
    /// Correo electrónico del último usuario
    /// </summary>
    public string Email { get; set; } = string.Empty;

    /// <summary>
    /// Indica si se debe recordar las credenciales del usuario
    /// </summary>
    public bool RememberCredentials { get; set; } = false;

    /// <summary>
    /// Fecha del último acceso
    /// </summary>
    public DateTime LastLogin { get; set; } = DateTime.UtcNow;
}

/// <summary>
/// Configuraciones visuales y de comportamiento de la UI
/// </summary>
public class UISettings
{
    /// <summary>
    /// Muestra u oculta las tarjetas de estadísticas en la vista de Cards
    /// </summary>
    public bool ShowStatisticsCards { get; set; } = true;

    /// <summary>
    /// Activa el modo compacto para las tarjetas de Cards
    /// </summary>
    public bool CompactCards { get; set; } = false;

    /// <summary>
    /// Muestra u oculta los adjuntos en tarjetas y diálogos
    /// </summary>
    public bool ShowAttachments { get; set; } = true;

    /// <summary>
    /// Modo de tema visual: "system" | "light" | "dark"
    /// </summary>
    public string ThemeMode { get; set; } = "system";

    /// <summary>
    /// Color de acento en formato HEX (por ejemplo, #0063B1)
    /// </summary>
    public string AccentColor { get; set; } = "#0063B1";
}