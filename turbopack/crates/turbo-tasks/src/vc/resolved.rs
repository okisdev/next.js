use std::{
    any::Any,
    fmt::Debug,
    future::IntoFuture,
    hash::{Hash, Hasher},
    ops::Deref,
};

use serde::{Deserialize, Serialize};

use crate::{
    debug::{ValueDebug, ValueDebugFormat, ValueDebugFormatString},
    trace::{TraceRawVcs, TraceRawVcsContext},
    vc::Vc,
    ResolveTypeError, Upcast, VcRead, VcTransparentRead, VcValueTrait, VcValueType,
};

#[derive(Serialize, Deserialize)]
#[serde(transparent, bound = "")]
pub struct ResolvedVc<T>
where
    T: ?Sized,
{
    pub(crate) node: Vc<T>,
}

impl<T> Copy for ResolvedVc<T> where T: ?Sized {}

impl<T> Clone for ResolvedVc<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Deref for ResolvedVc<T>
where
    T: ?Sized,
{
    type Target = Vc<T>;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<T> PartialEq<ResolvedVc<T>> for ResolvedVc<T>
where
    T: ?Sized,
{
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl<T> Eq for ResolvedVc<T> where T: ?Sized {}

impl<T> Hash for ResolvedVc<T>
where
    T: ?Sized,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node.hash(state);
    }
}

macro_rules! into_future {
    ($ty:ty) => {
        impl<T> IntoFuture for $ty
        where
            T: VcValueType,
        {
            type Output = <Vc<T> as IntoFuture>::Output;
            type IntoFuture = <Vc<T> as IntoFuture>::IntoFuture;
            fn into_future(self) -> Self::IntoFuture {
                (*self).into_future()
            }
        }
    };
}

into_future!(ResolvedVc<T>);
into_future!(&ResolvedVc<T>);
into_future!(&mut ResolvedVc<T>);

impl<T> ResolvedVc<T>
where
    T: VcValueType,
{
    // called by the `.resolved_cell()` method generated by the `#[turbo_tasks::value]` macro
    #[doc(hidden)]
    pub fn cell_private(inner: <T::Read as VcRead<T>>::Target) -> Self {
        Self {
            node: Vc::<T>::cell_private(inner),
        }
    }
}

impl<T, Inner, Repr> ResolvedVc<T>
where
    T: VcValueType<Read = VcTransparentRead<T, Inner, Repr>>,
    Inner: Any + Send + Sync,
    Repr: VcValueType,
{
    pub fn cell(inner: Inner) -> Self {
        Self {
            node: Vc::<T>::cell(inner),
        }
    }
}

impl<T> ResolvedVc<T>
where
    T: ?Sized,
{
    /// Upcasts the given `ResolvedVc<T>` to a `ResolvedVc<Box<dyn K>>`.
    ///
    /// See also: [`Vc::upcast`].
    #[inline(always)]
    pub fn upcast<K>(this: Self) -> ResolvedVc<K>
    where
        T: Upcast<K>,
        K: VcValueTrait + ?Sized,
    {
        ResolvedVc {
            node: Vc::upcast(this.node),
        }
    }
}

impl<T> ResolvedVc<T>
where
    T: VcValueTrait + ?Sized,
{
    /// Attempts to sidecast the given `Vc<Box<dyn T>>` to a `Vc<Box<dyn K>>`.
    ///
    /// Returns `None` if the underlying value type does not implement `K`.
    ///
    /// **Note:** if the trait `T` is required to implement `K`, use [`ResolvedVc::upcast`] instead.
    /// This provides stronger guarantees, removing the need for a [`Result`] return type.
    ///
    /// See also: [`Vc::try_resolve_sidecast`].
    pub async fn try_sidecast<K>(this: Self) -> Result<Option<ResolvedVc<K>>, ResolveTypeError>
    where
        K: VcValueTrait + ?Sized,
    {
        // must be async, as we must read the cell to determine the type
        Ok(Vc::try_resolve_sidecast(this.node)
            .await?
            .map(|node| ResolvedVc { node }))
    }

    /// Attempts to downcast the given `ResolvedVc<Box<dyn T>>` to a `ResolvedVc<K>`, where `K`
    /// is of the form `Box<dyn L>`, and `L` is a value trait.
    ///
    /// Returns `None` if the underlying value type is not a `K`.
    ///
    /// See also: [`Vc::try_resolve_downcast`].
    pub async fn try_downcast<K>(this: Self) -> Result<Option<ResolvedVc<K>>, ResolveTypeError>
    where
        K: Upcast<T> + VcValueTrait + ?Sized,
    {
        Ok(Vc::try_resolve_downcast(this.node)
            .await?
            .map(|node| ResolvedVc { node }))
    }

    /// Attempts to downcast the given `Vc<Box<dyn T>>` to a `Vc<K>`, where `K` is a value type.
    ///
    /// Returns `None` if the underlying value type is not a `K`.
    ///
    /// See also: [`Vc::try_resolve_downcast_type`].
    pub async fn try_downcast_type<K>(this: Self) -> Result<Option<ResolvedVc<K>>, ResolveTypeError>
    where
        K: Upcast<T> + VcValueType,
    {
        Ok(Vc::try_resolve_downcast_type(this.node)
            .await?
            .map(|node| ResolvedVc { node }))
    }
}

/// Generates an opaque debug representation of the [`ResolvedVc`] itself, but not the data inside
/// of it.
///
/// This is implemented to allow types containing [`ResolvedVc`] to implement the synchronous
/// [`Debug`] trait, but in most cases users should use the [`ValueDebug`] implementation to get a
/// string representation of the contents of the cell.
impl<T> Debug for ResolvedVc<T>
where
    T: ?Sized,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedVc")
            .field("node", &self.node.node)
            .finish()
    }
}

impl<T> TraceRawVcs for ResolvedVc<T>
where
    T: ?Sized,
{
    fn trace_raw_vcs(&self, trace_context: &mut TraceRawVcsContext) {
        TraceRawVcs::trace_raw_vcs(&self.node, trace_context);
    }
}

impl<T> ValueDebugFormat for ResolvedVc<T>
where
    T: Upcast<Box<dyn ValueDebug>> + Send + Sync + ?Sized,
{
    fn value_debug_format(&self, depth: usize) -> ValueDebugFormatString {
        self.node.value_debug_format(depth)
    }
}
