using Microsoft.AspNetCore.Components.Authorization;
using System.Security.Claims;
using Cardfile.Shared.Models;
using Microsoft.AspNetCore.Http;

namespace Cardfile.Shared.Services;

/// <summary>
/// Custom authentication state provider for Blazor Server.
/// It integrates with IAuthService and also checks the ASP.NET Core authentication cookie
/// to preserve the session across full page reloads or new circuits.
/// </summary>
public class CustomAuthenticationStateProvider : AuthenticationStateProvider
{
    private readonly IAuthService _authService;
    private readonly IHttpContextAccessor _httpContextAccessor;
    private AuthenticationState? _currentAuthenticationState;

    /// <summary>
    /// Initializes a new instance of the CustomAuthenticationStateProvider.
    /// </summary>
    /// <param name="authService">Authentication service that provides current user information.</param>
    /// <param name="httpContextAccessor">Accessor for the current HTTP context to read auth cookies.</param>
    public CustomAuthenticationStateProvider(IAuthService authService, IHttpContextAccessor httpContextAccessor)
    {
        _authService = authService;
        _httpContextAccessor = httpContextAccessor;

        // Subscribe to authentication state changes
        _authService.AuthenticationStateChanged += OnAuthenticationStateChanged;
    }

    /// <summary>
    /// Gets the current authentication state.
    /// Priority: ASP.NET Core auth cookie -> IAuthService persisted user -> anonymous.
    /// </summary>
    /// <returns>The current authentication state.</returns>
    public override async Task<AuthenticationState> GetAuthenticationStateAsync()
    {
        if (_currentAuthenticationState != null)
            return _currentAuthenticationState;

        // 1) Try to use the ASP.NET Core authentication cookie (persists across full reloads)
        var httpUser = _httpContextAccessor.HttpContext?.User;
        if (httpUser?.Identity?.IsAuthenticated == true)
        {
            _currentAuthenticationState = new AuthenticationState(httpUser);
            return _currentAuthenticationState;
        }

        // 2) Fallback to the IAuthService (persisted app settings logic)
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
    /// Creates a set of claims from a User instance.
    /// </summary>
    /// <param name="user">Domain user.</param>
    /// <returns>List of claims.</returns>
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
    /// Handles auth state changes raised by IAuthService.
    /// </summary>
    private void OnAuthenticationStateChanged(object? sender, AuthenticationStateChangedEventArgs e)
    {
        AuthenticationState newState;

        if (e.IsAuthenticated && e.User != null)
        {
            var claims = CreateClaimsFromUser(e.User);
            var identity = new ClaimsIdentity(claims, "custom");
            newState = new AuthenticationState(new ClaimsPrincipal(identity));
        }
        else
        {
            var identity = new ClaimsIdentity();
            newState = new AuthenticationState(new ClaimsPrincipal(identity));
        }

        _currentAuthenticationState = newState;
        NotifyAuthenticationStateChanged(Task.FromResult(newState));
    }

    /// <summary>
    /// Forces a refresh of the authentication state.
    /// </summary>
    public async Task RefreshAuthenticationStateAsync()
    {
        _currentAuthenticationState = null;
        var state = await GetAuthenticationStateAsync();
        NotifyAuthenticationStateChanged(Task.FromResult(state));
    }
}