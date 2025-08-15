using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services
{
    // Servicio para operaciones con Card
    public class CardService : ICardService
    {
        public Task<IEnumerable<Card>> GetAllAsync() => throw new NotImplementedException();
        public Task<Card?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(Card entity) => throw new NotImplementedException();
        public Task UpdateAsync(Card entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
    }

    // Servicio para operaciones con Tag
    public class TagService : ITagService
    {
        public Task<IEnumerable<Tag>> GetAllAsync() => throw new NotImplementedException();
        public Task<Tag?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(Tag entity) => throw new NotImplementedException();
        public Task UpdateAsync(Tag entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
    }

    // Servicio para operaciones con User
    public class UserService : IUserService
    {
        public Task<IEnumerable<User>> GetAllAsync() => throw new NotImplementedException();
        public Task<User?> GetByIdAsync(Guid id) => throw new NotImplementedException();
        public Task AddAsync(User entity) => throw new NotImplementedException();
        public Task UpdateAsync(User entity) => throw new NotImplementedException();
        public Task DeleteAsync(Guid id) => throw new NotImplementedException();
        public Task<User?> GetByUsernameAsync(string username) => throw new NotImplementedException();
        public Task<bool> ValidatePasswordAsync(string username, string password) => throw new NotImplementedException();
    }

    // Servicio para operaciones con AppConfig
    public class AppConfigService : IAppConfigService
    {
        private static AppConfig? _currentConfig;
        private static readonly List<AppConfig> _configs = new();

        /// <summary>
        /// Obtiene todas las configuraciones
        /// </summary>
        public Task<IEnumerable<AppConfig>> GetAllAsync()
        {
            return Task.FromResult<IEnumerable<AppConfig>>(_configs);
        }

        /// <summary>
        /// Obtiene una configuración por ID
        /// </summary>
        public Task<AppConfig?> GetByIdAsync(Guid id)
        {
            var config = _configs.FirstOrDefault(c => c.Id == id);
            return Task.FromResult(config);
        }

        /// <summary>
        /// Agrega una nueva configuración
        /// </summary>
        public Task AddAsync(AppConfig entity)
        {
            if (entity == null) throw new ArgumentNullException(nameof(entity));

            // Si es la primera configuración, la marcamos como actual
            if (_currentConfig == null)
            {
                _currentConfig = entity;
            }

            _configs.Add(entity);
            return Task.CompletedTask;
        }

        /// <summary>
        /// Actualiza una configuración existente
        /// </summary>
        public Task UpdateAsync(AppConfig entity)
        {
            if (entity == null) throw new ArgumentNullException(nameof(entity));

            var existingConfig = _configs.FirstOrDefault(c => c.Id == entity.Id);
            if (existingConfig != null)
            {
                var index = _configs.IndexOf(existingConfig);
                _configs[index] = entity;

                // Si es la configuración actual, actualizarla
                if (_currentConfig?.Id == entity.Id)
                {
                    _currentConfig = entity;
                }
            }

            return Task.CompletedTask;
        }

        /// <summary>
        /// Elimina una configuración por ID
        /// </summary>
        public Task DeleteAsync(Guid id)
        {
            var config = _configs.FirstOrDefault(c => c.Id == id);
            if (config != null)
            {
                _configs.Remove(config);

                // Si era la configuración actual, limpiarla
                if (_currentConfig?.Id == id)
                {
                    _currentConfig = _configs.FirstOrDefault();
                }
            }

            return Task.CompletedTask;
        }

        /// <summary>
        /// Obtiene la configuración actual de la aplicación
        /// </summary>
        public Task<AppConfig?> GetCurrentConfigAsync()
        {
            return Task.FromResult(_currentConfig);
        }

        /// <summary>
        /// Establece una configuración como la actual
        /// </summary>
        public Task SetCurrentConfigAsync(AppConfig config)
        {
            if (config == null) throw new ArgumentNullException(nameof(config));

            _currentConfig = config;
            return Task.CompletedTask;
        }

        /// <summary>
        /// Verifica si la aplicación ya ha sido configurada
        /// </summary>
        public Task<bool> IsConfiguredAsync()
        {
            return Task.FromResult(_currentConfig != null);
        }
    }
}
