use std::sync::Arc;
use observable::Observer;
use observable::Observable;
use subref::SubRef;
use subscriber::SubscriberImpl;
use subscriber::Subscriber;
use std::any::Any;
use std::marker::PhantomData;
use observable::FnCell;
use observable::ObserverHelper;

pub struct TapOp<V, Src, Obs>
{
    src: Src,
    obs: Obs,
    PhantomData: PhantomData<V>
}

pub trait ObservableTap<'x, Src, V:Clone+Send+Sync+'static, Obs> where
        for<'a> Obs: ObserverHelper<&'a V>+Send+Sync+'x+Clone,
        Src : Observable<'x, V>,
        Self: Sized,
{
    fn tap(self, o: Obs) -> TapOp<V, Src, Obs>;
}

impl<'x, Src, V:Clone+Send+Sync+'static, Obs> ObservableTap<'x, Src, V, Obs> for Src where
    V: Send+Sync+'static,
    for<'a> Obs: ObserverHelper<&'a V>+Send+Sync+'x+Clone,
    Src : Observable<'x, V>
{
    fn tap(self, o: Obs) -> TapOp<V, Src, Obs>
    {
        TapOp{ src: self, obs: o, PhantomData }
    }
}

impl<'x, V, Src, Obs> Observable<'x, V> for TapOp<V, Src, Obs> where
        V: Send+Sync+'static,
        for<'a> Obs: ObserverHelper<&'a V>+Send+Sync+'static+Clone,
        Src : Observable<'x, V>
{
    fn sub(&self, dest: impl Observer<V> + Send + Sync+'x) -> SubRef
    {
        let s = Subscriber::new(TapState{ obs: self.obs.clone() }, dest, false);
        s.do_sub(&self.src)
    }
}

struct TapState<Obs>
{
    obs: Obs
}

impl<'a, V, Obs,Dest> SubscriberImpl<V, TapState<Obs>> for Subscriber<'a, V, TapState<Obs>,Dest> where
    Dest: Observer<V>+Send+Sync+'a,
    for<'x> Obs: ObserverHelper<&'x V>+Send+Sync+'static,

{
    fn on_next(&self, v: V)
    {
        if self._dest._is_closed() {
            self.complete();
            return;
        }

        self._state.obs.next(&v);
        self._dest.next(v);

        if self._dest._is_closed() {
            self.complete();
        }
    }

    fn on_err(&self, e: Arc<Any + Send + Sync>)
    {
        self._state.obs.err(e.clone());

        self.do_unsub();
        self._dest.err(e);
    }

    fn on_comp(&self)
    {
        self._state.obs.complete();
        self.do_unsub();
        self._dest.complete();
    }
}

#[cfg(test)]
mod test
{
    use super::*;
    use fac::*;
    use observable::*;
    use observable::Observer;
    use op::*;

    #[test]
    fn basic()
    {
        rxfac::range(0..10)
            .take(5)
            .tap(|v:&i32| println!("{}", v))
            .take(100)
            .subf(|v| {});
    }
}