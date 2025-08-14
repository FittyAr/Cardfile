using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services
{
    // Interfaz para operaciones CRUD genéricas
    public interface IRepository<T> where T : class
    {
        Task<IEnumerable<T>> GetAllAsync();
        Task<T?> GetByIdAsync(Guid id);
        Task AddAsync(T entity);
        Task UpdateAsync(T entity);
        Task DeleteAsync(Guid id);
    }

    // Interfaz específica para Card
    public interface ICardService : IRepository<Card>
    {
        // Métodos adicionales específicos de Card pueden agregarse aquí
    }

    // Interfaz específica para Tag
    public interface ITagService : IRepository<Tag>
    {
        // Métodos adicionales específicos de Tag pueden agregarse aquí
    }

    // Interfaz específica para User
    public interface IUserService : IRepository<User>
    {
        Task<User?> GetByUsernameAsync(string username);
        Task<bool> ValidatePasswordAsync(string username, string password);
    }

    // Interfaz específica para AppConfig
    public interface IAppConfigService : IRepository<AppConfig>
    {
        Task<AppConfig?> GetCurrentConfigAsync();
    }
}
