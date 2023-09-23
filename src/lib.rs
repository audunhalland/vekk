use std::ops::{Deref, DerefMut};

use thin_vec::ThinVec;

pub trait Array {
    type Item: Default;
    const CAPACITY: usize;

    fn as_slice(&self) -> &[Self::Item];
    fn as_slice_mut(&mut self) -> &mut [Self::Item];
    fn default() -> Self;
}

impl<T: Default, const N: usize> Array for [T; N] {
    type Item = T;
    const CAPACITY: usize = N;

    #[inline]
    fn as_slice(&self) -> &[Self::Item] {
        self
    }

    #[inline]
    fn as_slice_mut(&mut self) -> &mut [Self::Item] {
        self
    }

    #[inline]
    fn default() -> Self {
        [(); N].map(|_| T::default())
    }
}

pub struct Vekk<A: Array> {
    repr: Repr<A>,
}

enum Repr<A: Array> {
    Inline { len: u16, array: A },
    Heap(ThinVec<A::Item>),
}

impl<A: Array> Vekk<A> {
    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::Inline { len, .. } => *len as usize,
            Repr::Heap(vec) => vec.len(),
        }
    }

    pub fn as_slice(&self) -> &[A::Item] {
        self.deref()
    }

    pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
        self.deref_mut()
    }

    pub fn push(&mut self, item: A::Item) {
        match &mut self.repr {
            Repr::Inline { len, array } => {
                if *len as usize == A::CAPACITY {
                    let mut vec = ThinVec::with_capacity(A::CAPACITY + 1);
                    for item in array.as_slice_mut() {
                        let item = core::mem::take(item);
                        vec.push(item);
                    }
                    vec.push(item);
                    self.repr = Repr::Heap(vec);
                } else {
                    array.as_slice_mut()[*len as usize] = item;
                    *len += 1;
                }
            }
            Repr::Heap(vec) => {
                vec.push(item);
            }
        }
    }

    pub fn pop(&mut self) -> Option<A::Item> {
        match &mut self.repr {
            Repr::Inline { len, array } => {
                if *len > 0 {
                    let item = core::mem::take(&mut array.as_slice_mut()[(*len - 1) as usize]);
                    *len -= 1;
                    Some(item)
                } else {
                    None
                }
            }
            Repr::Heap(vec) => {
                // Currently does not switch back to inline representation
                vec.pop()
            }
        }
    }
}

impl<A: Array> core::ops::Deref for Vekk<A> {
    type Target = [A::Item];

    fn deref(&self) -> &Self::Target {
        match &self.repr {
            Repr::Inline { len, array } => &array.as_slice()[..(*len as usize)],
            Repr::Heap(vec) => vec.as_slice(),
        }
    }
}

impl<A: Array> core::ops::DerefMut for Vekk<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.repr {
            Repr::Inline { len, array } => &mut array.as_slice_mut()[..(*len as usize)],
            Repr::Heap(vec) => vec.as_mut_slice(),
        }
    }
}

impl<A: Array> Default for Vekk<A> {
    fn default() -> Self {
        Self {
            repr: Repr::Inline {
                len: 0,
                array: A::default(),
            },
        }
    }
}

impl<A: Array> FromIterator<A::Item> for Vekk<A> {
    fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        match iter.size_hint() {
            (_, Some(upper)) if upper > A::CAPACITY => {
                let vec = ThinVec::from_iter(iter);
                Self {
                    repr: Repr::Heap(vec),
                }
            }
            (_, upper) => {
                let mut array = A::default();
                let slice = array.as_slice_mut();
                let mut len = 0;

                while let Some(item) = iter.next() {
                    if len >= A::CAPACITY || len >= u16::MAX as usize {
                        let mut vec = ThinVec::with_capacity(upper.unwrap_or(A::CAPACITY));
                        for item in slice {
                            vec.push(core::mem::take(item));
                        }
                        vec.extend(iter);

                        return Self {
                            repr: Repr::Heap(vec),
                        };
                    }

                    slice[len] = item;
                    len += 1;
                }

                Self {
                    repr: Repr::Inline {
                        len: len as u16,
                        array,
                    },
                }
            }
        }
    }
}

impl<A: Array> IntoIterator for Vekk<A> {
    type Item = A::Item;
    type IntoIter = Iter<A>;

    fn into_iter(self) -> Self::IntoIter {
        match self.repr {
            Repr::Inline { len, array } => {
                Iter(IterRepr::Inline(InlineIter { pos: 0, len, array }))
            }
            Repr::Heap(vec) => Iter(IterRepr::Heap(vec.into_iter())),
        }
    }
}

pub struct Iter<A: Array>(IterRepr<A>);

enum IterRepr<A: Array> {
    Inline(InlineIter<A>),
    Heap(<ThinVec<A::Item> as IntoIterator>::IntoIter),
}

impl<A: Array> Iterator for Iter<A> {
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IterRepr::Inline(iter) => iter.next(),
            IterRepr::Heap(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            IterRepr::Inline(inline) => inline.size_hint(),
            IterRepr::Heap(heap) => heap.size_hint(),
        }
    }
}

struct InlineIter<A: Array> {
    pos: u16,
    len: u16,
    array: A,
}

impl<A: Array> Iterator for InlineIter<A> {
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.len {
            None
        } else {
            let item = core::mem::take(&mut self.array.as_slice_mut()[self.pos as usize]);
            self.pos += 1;
            Some(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.len - self.pos) as usize;
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn size() {
        assert_eq!(size_of::<Vekk<[usize; 1]>>(), 16);

        // Would like this to be 8 bytes, but can't manage to trick rustc into doing that
        assert_eq!(size_of::<Vekk<[u32; 1]>>(), 16);
    }

    #[test]
    fn zero() {
        let v: Vekk<[u32; 0]> = [].into_iter().collect();
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.as_slice(), &[]);
        assert_eq!(v.iter().collect::<Vec<_>>(), Vec::<&u32>::new());
        assert_eq!(v.into_iter().collect::<Vec<_>>(), Vec::<u32>::new());

        let v: Vekk<[u32; 0]> = [42].into_iter().collect();
        assert!(matches!(v.repr, Repr::Heap(_)));
        assert_eq!(v.as_slice(), &[42]);
        assert_eq!(v.iter().collect::<Vec<_>>(), vec![&42]);
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec![42]);
    }

    #[test]
    fn one() {
        let v: Vekk<[u32; 1]> = [].into_iter().collect();
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.as_slice(), &[]);
        assert_eq!(v.iter().collect::<Vec<_>>(), Vec::<&u32>::new());
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec![]);

        let v: Vekk<[u32; 1]> = [42].into_iter().collect();
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.as_slice(), &[42]);
        assert_eq!(v.iter().collect::<Vec<_>>(), vec![&42]);
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec![42]);

        let v: Vekk<[u32; 1]> = [1, 2].into_iter().collect();
        assert!(matches!(v.repr, Repr::Heap(_)));
        assert_eq!(v.as_slice(), &[1, 2]);
        assert_eq!(v.iter().collect::<Vec<_>>(), vec![&1, &2]);
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn push_pop() {
        let mut v: Vekk<[u32; 1]> = Default::default();
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.len(), 0);
        assert_eq!(v.pop(), None);

        v.push(1);
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.len(), 1);
        assert_eq!(v.as_slice(), &[1]);

        assert_eq!(v.pop(), Some(1));
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.len(), 0);
        assert_eq!(v.as_slice(), &[]);

        v.push(1);
        assert!(matches!(v.repr, Repr::Inline { .. }));
        assert_eq!(v.as_slice(), &[1]);

        v.push(2);
        assert!(matches!(v.repr, Repr::Heap(_)));
        assert_eq!(v.as_slice(), &[1, 2]);

        assert_eq!(v.pop(), Some(2));
        assert!(matches!(v.repr, Repr::Heap(_)));
        assert_eq!(v.as_slice(), &[1]);
    }
}
