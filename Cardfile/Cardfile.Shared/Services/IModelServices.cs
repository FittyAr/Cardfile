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

    // Interfaz específica para AppConfig
    public interface IAppConfigService : IRepository<AppConfig>
    {
        /// <summary>
        /// Obtiene la configuración actual de la aplicación
        /// </summary>
        Task<AppConfig?> GetCurrentConfigAsync();

        /// <summary>
        /// Establece una configuración como la actual
        /// </summary>
        Task SetCurrentConfigAsync(AppConfig config);

        /// <summary>
        /// Verifica si la aplicación ya ha sido configurada
        /// </summary>
        Task<bool> IsConfiguredAsync();
    }

    // Interfaz específica para CardAttachment
    public interface ICardAttachmentService : IRepository<CardAttachment>
    {
        /// <summary>
        /// Obtiene todos los archivos adjuntos de una tarjeta específica
        /// </summary>
        Task<IEnumerable<CardAttachment>> GetByCardIdAsync(Guid cardId);

        /// <summary>
        /// Obtiene el contenido de un archivo adjunto específico
        /// </summary>
        Task<byte[]?> GetFileDataAsync(Guid attachmentId);

        /// <summary>
        /// Verifica si el tipo MIME es válido para subir
        /// </summary>
        bool IsValidFileType(string contentType);

        /// <summary>
        /// Verifica si el tamaño del archivo es válido
        /// </summary>
        bool IsValidFileSize(long fileSize);
    }
}
