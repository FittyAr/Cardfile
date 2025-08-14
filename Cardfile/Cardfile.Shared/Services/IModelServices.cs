using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Cardfile.Shared.Models;

namespace Cardfile.Shared.Services
{
    // Interfaz para operaciones CRUD gen�ricas
    public interface IRepository<T> where T : class
    {
        Task<IEnumerable<T>> GetAllAsync();
        Task<T?> GetByIdAsync(Guid id);
        Task AddAsync(T entity);
        Task UpdateAsync(T entity);
        Task DeleteAsync(Guid id);
    }

    // Interfaz espec�fica para Card
    public interface ICardService : IRepository<Card>
    {
        // M�todos adicionales espec�ficos de Card pueden agregarse aqu�
    }

    // Interfaz espec�fica para Tag
    public interface ITagService : IRepository<Tag>
    {
        // M�todos adicionales espec�ficos de Tag pueden agregarse aqu�
    }

    // Interfaz espec�fica para User
    public interface IUserService : IRepository<User>
    {
        Task<User?> GetByUsernameAsync(string username);
        Task<bool> ValidatePasswordAsync(string username, string password);
    }

    // Interfaz espec�fica para AppConfig
    public interface IAppConfigService : IRepository<AppConfig>
    {
        Task<AppConfig?> GetCurrentConfigAsync();
    }
}
