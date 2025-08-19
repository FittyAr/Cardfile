using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services;

/// <summary>
/// Interfaz para el servicio de autenticación
/// Maneja el login, logout, registro y gestión del estado de autenticación
/// </summary>
public interface IAuthService
{
    /// <summary>
    /// Intenta autenticar un usuario con sus credenciales
    /// </summary>
    /// <param name="username">Nombre de usuario</param>
    /// <param name="password">Contraseña</param>
    /// <returns>Usuario autenticado o null si falla la autenticación</returns>
    Task<User?> LoginAsync(string username, string password);

    /// <summary>
    /// Registra un nuevo usuario en el sistema
    /// </summary>
    /// <param name="username">Nombre de usuario</param>
    /// <param name="email">Correo electrónico</param>
    /// <param name="password">Contraseña</param>
    /// <returns>True si el registro fue exitoso</returns>
    Task<bool> RegisterAsync(string username, string email, string password);

    /// <summary>
    /// Cierra la sesión del usuario actual
    /// </summary>
    Task LogoutAsync();

    /// <summary>
    /// Obtiene el usuario actualmente autenticado
    /// </summary>
    /// <returns>Usuario actual o null si no está autenticado</returns>
    Task<User?> GetCurrentUserAsync();

    /// <summary>
    /// Verifica si hay un usuario autenticado
    /// </summary>
    /// <returns>True si hay un usuario autenticado</returns>
    Task<bool> IsAuthenticatedAsync();

    /// <summary>
    /// Cambia la contraseña del usuario actual
    /// </summary>
    /// <param name="currentPassword">Contraseña actual</param>
    /// <param name="newPassword">Nueva contraseña</param>
    /// <returns>True si el cambio fue exitoso</returns>
    Task<bool> ChangePasswordAsync(string currentPassword, string newPassword);

    /// <summary>
    /// Evento que se dispara cuando cambia el estado de autenticación
    /// </summary>
    event EventHandler<AuthenticationStateChangedEventArgs>? AuthenticationStateChanged;
}

/// <summary>
/// Argumentos del evento de cambio de estado de autenticación
/// </summary>
public class AuthenticationStateChangedEventArgs : EventArgs
{
    /// <summary>
    /// Usuario actual después del cambio de estado
    /// </summary>
    public User? User { get; set; }

    /// <summary>
    /// Indica si el usuario está autenticado
    /// </summary>
    public bool IsAuthenticated { get; set; }
}