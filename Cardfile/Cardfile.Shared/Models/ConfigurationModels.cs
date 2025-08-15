namespace Cardfile.Shared.Models;

/// <summary>
/// Configuración de conexión a base de datos
/// </summary>
public class DatabaseConnectionConfig
{
    public string Host { get; set; } = "localhost";
    public int Port { get; set; } = 3306;
    public string DatabaseName { get; set; } = "cardfile_db";
    public string Username { get; set; } = "root";
    public string Password { get; set; } = string.Empty;
}

/// <summary>
/// Configuración de usuario inicial
/// </summary>
public class InitialUserConfig
{
    public string Username { get; set; } = string.Empty;
    public string Email { get; set; } = string.Empty;
    public string Password { get; set; } = string.Empty;
}