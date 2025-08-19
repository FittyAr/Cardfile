using Microsoft.AspNetCore.Components.Authorization;
using System.Security.Claims;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services;

/// <summary>
/// Proveedor personalizado de estado de autenticación
/// Integra con AuthService para gestionar el estado de autenticación a nivel de la aplicación
/// </summary>
public class CustomAuthenticationStateProvider : AuthenticationStateProvider
{
    private readonly IAuthService _authService;
    private AuthenticationState? _currentAuthenticationState;

    public CustomAuthenticationStateProvider(IAuthService authService)
    {
        _authService = authService;
        
        // Suscribirse a cambios en el estado de autenticación
        _authService.AuthenticationStateChanged += OnAuthenticationStateChanged;
    }

    /// <summary>
    /// Obtiene el estado de autenticación actual
    /// </summary>
    /// <returns>Estado de autenticación actual</returns>
    public override async Task<AuthenticationState> GetAuthenticationStateAsync()
    {
        if (_currentAuthenticationState != null)
            return _currentAuthenticationState;

        // Intentar obtener el usuario actual
        var user = await _authService.GetCurrentUserAsync();
        
        if (user != null)
        {
            var claims = CreateClaimsFromUser(user);
            var identity = new ClaimsIdentity(claims, "custom");
            _currentAuthenticationState = new AuthenticationState(new ClaimsPrincipal(identity));
        }
        else
        {
            var identity = new ClaimsIdentity();
            _currentAuthenticationState = new AuthenticationState(new ClaimsPrincipal(identity));
        }

        return _currentAuthenticationState;
    }

    /// <summary>
    /// Crea los claims de seguridad a partir de un usuario
    /// </summary>
    /// <param name="user">Usuario del que crear los claims</param>
    /// <returns>Lista de claims</returns>
    private static List<Claim> CreateClaimsFromUser(User user)
    {
        return new List<Claim>
        {
            new Claim(ClaimTypes.NameIdentifier, user.Id.ToString()),
            new Claim(ClaimTypes.Name, user.Username),
            new Claim(ClaimTypes.Email, user.Email ?? string.Empty),
            new Claim("IsActive", user.IsActive.ToString()),
            new Claim("CreatedAt", user.CreatedAt.ToString("O"))
        };
    }

    /// <summary>
    /// Maneja los cambios en el estado de autenticación del AuthService
    /// </summary>
    /// <param name="sender">Origen del evento</param>
    /// <param name="e">Argumentos del evento</param>
    private void OnAuthenticationStateChanged(object? sender, AuthenticationStateChangedEventArgs e)
    {
        Console.WriteLine($"DEBUG CustomAuthenticationStateProvider: Recibido cambio de estado - IsAuthenticated: {e.IsAuthenticated}, User: {e.User?.Username ?? "null"}");
        AuthenticationState newState;

        if (e.IsAuthenticated && e.User != null)
        {
            Console.WriteLine($"DEBUG CustomAuthenticationStateProvider: Creando estado autenticado para {e.User.Username}");
            var claims = CreateClaimsFromUser(e.User);
            var identity = new ClaimsIdentity(claims, "custom");
            newState = new AuthenticationState(new ClaimsPrincipal(identity));
        }
        else
        {
            Console.WriteLine($"DEBUG CustomAuthenticationStateProvider: Creando estado no autenticado");
            var identity = new ClaimsIdentity();
            newState = new AuthenticationState(new ClaimsPrincipal(identity));
        }

        _currentAuthenticationState = newState;
        Console.WriteLine($"DEBUG CustomAuthenticationStateProvider: Notificando cambio de estado");
        NotifyAuthenticationStateChanged(Task.FromResult(newState));
        Console.WriteLine($"DEBUG CustomAuthenticationStateProvider: Notificación completada");
    }

    /// <summary>
    /// Fuerza la actualización del estado de autenticación
    /// </summary>
    public async Task RefreshAuthenticationStateAsync()
    {
        _currentAuthenticationState = null;
        var state = await GetAuthenticationStateAsync();
        NotifyAuthenticationStateChanged(Task.FromResult(state));
    }
}