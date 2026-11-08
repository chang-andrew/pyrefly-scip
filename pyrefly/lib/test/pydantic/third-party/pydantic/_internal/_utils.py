"""Bucket of reusable internal utilities.

This test fixture keeps only `deprecated_instance_property` from pydantic's module: it
decorates `BaseModel.model_fields` and `BaseModel.model_computed_fields`, and an unresolved
decorator would leave them typed as class methods.
"""

from __future__ import annotations as _annotations

from typing import TYPE_CHECKING, Callable, Generic, TypeVar, overload

from typing_extensions import deprecated

if TYPE_CHECKING:
    from ..main import BaseModel

_ModelT = TypeVar('_ModelT', bound='BaseModel')
_RT = TypeVar('_RT')


class deprecated_instance_property(Generic[_ModelT, _RT]):
    """A decorator exposing the decorated class method as a property, with a warning on instance access.

    This decorator takes a class method defined on the `BaseModel` class and transforms it into
    an attribute. The attribute can be accessed on both the class and instances of the class. If accessed
    via an instance, a deprecation warning is emitted stating that instance access will be removed in V3.
    """

    def __init__(self, fget: Callable[[type[_ModelT]], _RT], /) -> None:
        # Note: fget should be a classmethod:
        self.fget = fget

    @overload
    def __get__(self, instance: None, objtype: type[_ModelT]) -> _RT: ...
    @overload
    @deprecated(
        'Accessing this attribute on the instance is deprecated, and will be removed in Pydantic V3. '
        'Instead, you should access this attribute from the model class.',
        category=None,
    )
    def __get__(self, instance: _ModelT, objtype: type[_ModelT]) -> _RT: ...
    def __get__(self, instance: _ModelT | None, objtype: type[_ModelT]) -> _RT:
        return self.fget.__get__(instance, objtype)()
