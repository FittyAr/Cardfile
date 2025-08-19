using Cardfile.Shared.Models;
using Microsoft.AspNetCore.Components;
using Microsoft.AspNetCore.Http;

namespace Cardfile.Shared.Services;

/// <summary>
/// Implementación del servicio de autenticación
/// Maneja el login, logout, registro y gestión del estado de autenticación usando LocalStorage
/// </summary>
public class AuthService : IAuthService
{
    private readonly IUserService _userService;
    private readonly IAppSettingsService _appSettingsService;
    private readonly NavigationManager _navigationManager;
    private readonly IHttpContextAccessor _httpContextAccessor;

    private User? _currentUser;
    private const string UserStorageKey = "cardfile_current_user";

    public event EventHandler<AuthenticationStateChangedEventArgs>? AuthenticationStateChanged;

    public AuthService(
        IUserService userService, 
        IAppSettingsService appSettingsService,
        NavigationManager navigationManager,
        IHttpContextAccessor httpContextAccessor)
    {
        _userService = userService;
        _appSettingsService = appSettingsService;
        _navigationManager = navigationManager;
        _httpContextAccessor = httpContextAccessor;
    }

    /// <summary>
    /// Intenta autenticar un usuario con sus credenciales
    /// </summary>
    /// <param name="username">Nombre de usuario</param>
    /// <param name="password">Contraseña</param>
    /// <returns>Usuario autenticado o null si falla la autenticación</returns>
    public async Task<User?> LoginAsync(string username, string password)
    {
        Console.WriteLine($"DEBUG LoginAsync: Iniciando login para usuario: {username}");

        if (string.IsNullOrWhiteSpace(username) || string.IsNullOrWhiteSpace(password))
        {
            Console.WriteLine($"DEBUG LoginAsync: Credenciales vacías");
            return null;
        }

        var user = await _userService.GetByUsernameAsync(username);
        if (user == null || !user.IsActive)
        {
            Console.WriteLine($"DEBUG LoginAsync: Usuario no encontrado o inactivo");
            return null;
        }

        // Verificar la contraseña usando BCrypt
        if (!BCrypt.Net.BCrypt.Verify(password, user.PasswordHash))
        {
            Console.WriteLine($"DEBUG LoginAsync: Contraseña incorrecta");
            return null;
        }

        Console.WriteLine($"DEBUG LoginAsync: Contraseña válida, estableciendo _currentUser");
        // Actualizar el estado del usuario actual
        _currentUser = user;

        // Guardar información de login
        Console.WriteLine($"DEBUG LoginAsync: Guardando LastUserInfo");
        await _appSettingsService.UpdateLastUserAsync(new LastUserInfo
        {
            Username = user.Username,
            Email = user.Email ?? string.Empty,
            RememberCredentials = false, // No persistir sesión por defecto
            LastLogin = DateTime.UtcNow
        });

        // Notificar cambio de estado de autenticación inmediatamente después del login
        Console.WriteLine($"DEBUG LoginAsync: Notificando cambio de estado de autenticación");
        AuthenticationStateChanged?.Invoke(this, new AuthenticationStateChangedEventArgs
        {
            User = user,
            IsAuthenticated = true
        });

        Console.WriteLine($"DEBUG LoginAsync: Login completado exitosamente para: {user.Username}");
        return user;
    }

    /// <summary>
    /// Registra un nuevo usuario en el sistema
    /// </summary>
    /// <param name="username">Nombre de usuario</param>
    /// <param name="email">Correo electrónico</param>
    /// <param name="password">Contraseña</param>
    /// <returns>True si el registro fue exitoso</returns>
    public async Task<bool> RegisterAsync(string username, string email, string password)
    {
        try
        {
            // Verificar si el usuario ya existe
            var existingUser = await _userService.GetByUsernameAsync(username);
            if (existingUser != null)
                return false; // Usuario ya existe

            // Crear nuevo usuario
            var newUser = new User
            {
                Id = Guid.NewGuid(),
                Username = username,
                Email = email,
                PasswordHash = BCrypt.Net.BCrypt.HashPassword(password),
                IsActive = true,
                CreatedAt = DateTime.UtcNow
            };

            // Guardar el usuario
            await _userService.AddAsync(newUser);

            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>
    /// Cierra la sesión del usuario actual
    /// </summary>
    public async Task LogoutAsync()
    {
        Console.WriteLine($"DEBUG LogoutAsync: Iniciando logout");
        Console.WriteLine($"DEBUG LogoutAsync: _currentUser antes del logout: {_currentUser?.Username ?? "null"}");

        _currentUser = null;
        Console.WriteLine($"DEBUG LogoutAsync: _currentUser establecido a null");

        // Limpiar información guardada del usuario creando un LastUserInfo vacío
        Console.WriteLine($"DEBUG LogoutAsync: Limpiando LastUserInfo");
        await _appSettingsService.UpdateLastUserAsync(new LastUserInfo
        {
            Username = string.Empty,
            Email = string.Empty,
            RememberCredentials = false,
            LastLogin = DateTime.UtcNow // Mantener la fecha actual para evitar problemas de validación
        });

        // En Blazor Server, no manejamos cookies aquí
        // El CustomAuthenticationStateProvider manejará el estado de autenticación

        // Notificar cambio de estado
        Console.WriteLine($"DEBUG LogoutAsync: Notificando cambio de estado de autenticación");
        AuthenticationStateChanged?.Invoke(this, new AuthenticationStateChangedEventArgs
        {
            User = null,
            IsAuthenticated = false
        });

        // Redirigir a login
        Console.WriteLine($"DEBUG LogoutAsync: Logout completado, redirigiendo a login");
        _navigationManager.NavigateTo("/login", true);
    }

    /// <summary>
    /// Obtiene el usuario actualmente autenticado
    /// </summary>
    /// <returns>Usuario actual o null si no está autenticado</returns>
    public async Task<User?> GetCurrentUserAsync()
    {
        // Siempre consultar la configuración persistente en lugar de confiar en _currentUser
        // debido al ciclo de vida Scoped del servicio en Blazor Server
        var lastUser = await _appSettingsService.GetLastUserAsync();
        if (lastUser != null)
        {
            Console.WriteLine($"DEBUG GetCurrentUserAsync: LastUser => Username='{lastUser.Username}', RememberCredentials={lastUser.RememberCredentials}, LastLogin={lastUser.LastLogin:O}");
        }

        if (lastUser != null && lastUser.RememberCredentials && !string.IsNullOrEmpty(lastUser.Username))
        {
            // Verificar si el usuario existe y está activo
            var user = await _userService.GetByUsernameAsync(lastUser.Username);
            if (user != null && user.IsActive)
            {
                // Verificar que el login no sea muy antiguo (24 horas)
                if (lastUser.LastLogin > DateTime.UtcNow.AddHours(-24))
                {
                    // Actualizar _currentUser para optimización en la misma instancia
                    _currentUser = user;
                    return user;
                }
            }
        }

        return null;
    }

    /// <summary>
    /// Verifica si hay un usuario autenticado
    /// </summary>
    /// <returns>True si hay un usuario autenticado</returns>
    public async Task<bool> IsAuthenticatedAsync()
    {
        Console.WriteLine($"DEBUG IsAuthenticatedAsync: _currentUser = {_currentUser?.Username ?? "null"}");

        // Si hay un usuario en memoria, está autenticado
        if (_currentUser != null)
        {
            Console.WriteLine($"DEBUG IsAuthenticatedAsync: Usuario en memoria encontrado: {_currentUser.Username}");
            return true;
        }

        // Si no hay usuario en memoria, verificar persistencia
        var user = await GetCurrentUserAsync();
        var isAuthenticated = user != null;
        Console.WriteLine($"DEBUG IsAuthenticatedAsync: Resultado de verificación persistente: {isAuthenticated}");
        return isAuthenticated;
    }

    /// <summary>
    /// Cambia la contraseña del usuario actual
    /// </summary>
    /// <param name="currentPassword">Contraseña actual</param>
    /// <param name="newPassword">Nueva contraseña</param>
    /// <returns>True si el cambio fue exitoso</returns>
    public async Task<bool> ChangePasswordAsync(string currentPassword, string newPassword)
    {
        try
        {
            var user = await GetCurrentUserAsync();
            if (user == null)
                return false;

            // Validar contraseña actual
            var isValid = await _userService.ValidatePasswordAsync(user.Username, currentPassword);
            if (!isValid)
                return false;

            // Actualizar contraseña
            user.PasswordHash = BCrypt.Net.BCrypt.HashPassword(newPassword);
            await _userService.UpdateAsync(user);

            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }
}