"""
Gestión de estado para la vista de tarjetas

Este módulo maneja el estado de las tarjetas y el autoguardado:
- Estado de selección de tarjetas
- Detección de cambios pendientes
- Autoguardado automático
- Debouncing de cambios

Uso:
    from View.components.card_state import CardState
    
    state = CardState()
    state.select_ficha(ficha)
    state.mark_as_modified()
    await state.save_if_needed(session, config)
"""
import asyncio
from typing import Optional
from dataclasses import dataclass, field


@dataclass
class CardState:
    """
    Clase que mantiene el estado de la vista de tarjetas.
    
    Attributes:
        selected_ficha: Ficha actualmente seleccionada (o None)
        last_saved_value: Último valor guardado (para detectar cambios)
        has_unsaved_changes: Si hay cambios pendientes de guardar
        debounce_task: Task de debounce para guardar cambios
        autosave_task: Task de autoguardado periódico
    """
    selected_ficha: Optional[object] = None
    last_saved_value: str = ""
    has_unsaved_changes: bool = False
    debounce_task: Optional[asyncio.Task] = None
    autosave_task: Optional[asyncio.Task] = None
    fichas_list: list = field(default_factory=list)
    unlocked_fichas: set = field(default_factory=set)
    relock_tasks: dict = field(default_factory=dict)
    
    def select_ficha(self, ficha):
        """
        Selecciona una nueva ficha.
        
        Args:
            ficha: La ficha a seleccionar
        """
        self.selected_ficha = ficha
        self.last_saved_value = ficha.descripcion if ficha else ""
        self.has_unsaved_changes = False
    
    def deselect(self):
        """Deselecciona la ficha actual"""
        self.selected_ficha = None
        self.last_saved_value = ""
        self.has_unsaved_changes = False
    
    def mark_as_modified(self):
        """Marca que hay cambios pendientes"""
        self.has_unsaved_changes = True
    
    def mark_as_saved(self, value: str):
        """
        Marca como guardado y actualiza el último valor.
        
        Args:
            value: Valor que se acaba de guardar
        """
        self.has_unsaved_changes = False
        self.last_saved_value = value
    
    def has_fichas(self) -> bool:
        """
        Verifica si hay fichas en la lista.
        
        Returns:
            True si hay al menos una ficha
        """
        return bool(self.fichas_list)
    
    def is_ficha_selected(self) -> bool:
        """
        Verifica si hay una ficha seleccionada.
        
        Returns:
            True si hay una ficha seleccionada
        """
        return self.selected_ficha is not None
    
    def editor_should_be_enabled(self) -> bool:
        """
        Determina si el editor debe estar habilitado.
        
        Returns:
            True si el editor debe estar habilitado
        """
        return self.has_fichas() and self.is_ficha_selected()
    
    async def cleanup(self):
        """Limpia las tasks pendientes al desmontar la vista"""
        if self.debounce_task and not self.debounce_task.done():
            self.debounce_task.cancel()
            try:
                await self.debounce_task
            except asyncio.CancelledError:
                pass
        
        if self.autosave_task and not self.autosave_task.done():
            self.autosave_task.cancel()
            try:
                await self.autosave_task
            except asyncio.CancelledError:
                pass

        for task in list(self.relock_tasks.values()):
            if task and not task.done():
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
