use std::ops::{Deref, DerefMut};

use thin_vec::ThinVec;

pub mod iter;

pub trait Array: IntoIterator {
    const CAPACITY: usize;

    fn default() -> Self;
    fn as_slice(&self) -> &[Self::Item];
    fn as_slice_mut(&mut self) -> &mut [Self::Item];
}

impl<T: Default, const N: usize> Array for [T; N] {
    const CAPACITY: usize = N;

    #[inline]
    fn default() -> Self {
        [(); N].map(|_| T::default())
    }

    #[inline]
    fn as_slice(&self) -> &[Self::Item] {
        self
    }

    #[inline]
    fn as_slice_mut(&mut self) -> &mut [Self::Item] {
        self
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

    pub fn push(&mut self, item: A::Item)
    where
        A::Item: Default,
    {
        self.push_inner(item);
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = A::Item>)
    where
        A::Item: Default,
    {
        for item in iter {
            self.push_inner(item);
        }
    }

    pub fn pop(&mut self) -> Option<A::Item>
    where
        A::Item: Default,
    {
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

    pub fn insert(&mut self, index: usize, element: A::Item)
    where
        A::Item: Default,
    {
        match &mut self.repr {
            Repr::Inline { len, array } => {
                if (*len as usize) == Self::inline_capacity() {
                    let mut vec = Self::thinvec_from_array(array, Self::inline_capacity() + 1);
                    vec.insert(index, element);
                    self.repr = Repr::Heap(vec);
                } else {
                    let slice = array.as_slice_mut();
                    for idx in index..(*len as usize) {
                        slice.swap(idx, idx + 1);
                    }
                    slice[index] = element;
                    *len += 1;
                }
            }
            Repr::Heap(vec) => {
                vec.insert(index, element);
            }
        }
    }

    fn inline_capacity() -> usize {
        core::cmp::min(A::CAPACITY, u16::MAX as usize)
    }

    #[inline]
    fn thinvec_from_array(array: &mut A, capacity: usize) -> ThinVec<A::Item>
    where
        A::Item: Default,
    {
        let mut vec = ThinVec::with_capacity(capacity);
        for item in array.as_slice_mut() {
            let item = core::mem::take(item);
            vec.push(item);
        }
        vec
    }

    #[inline]
    pub fn push_inner(&mut self, item: A::Item)
    where
        A::Item: Default,
    {
        match &mut self.repr {
            Repr::Inline { len, array } => {
                if *len as usize == Self::inline_capacity() {
                    let mut vec = Self::thinvec_from_array(array, Self::inline_capacity() + 1);
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

impl<A: Array> Clone for Vekk<A>
where
    A: Clone,
    A::Item: Clone,
{
    fn clone(&self) -> Self {
        Self {
            repr: self.repr.clone(),
        }
    }
}

impl<A: Array> Clone for Repr<A>
where
    A: Clone,
    A::Item: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Inline { len, array } => Self::Inline {
                len: *len,
                array: array.clone(),
            },
            Self::Heap(vec) => Self::Heap(vec.clone()),
        }
    }
}

impl<A: Array> From<A> for Vekk<A> {
    fn from(value: A) -> Self {
        Self {
            repr: Repr::Inline {
                len: A::CAPACITY as u16,
                array: value,
            },
        }
    }
}

impl<A: Array> From<Vec<A::Item>> for Vekk<A>
where
    A::Item: Default,
{
    fn from(value: Vec<A::Item>) -> Self {
        value.into_iter().collect()
    }
}

impl<A: Array> FromIterator<A::Item> for Vekk<A>
where
    A::Item: Default,
{
    fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        match iter.size_hint() {
            (_, Some(upper)) if upper > A::CAPACITY => Self {
                repr: Repr::Heap(ThinVec::from_iter(iter)),
            },
            _ => {
                let mut array = A::default();
                let slice = array.as_slice_mut();
                let mut len = 0;

                let inline_capacity = core::cmp::min(A::CAPACITY, u16::MAX as usize);

                while let Some(item) = iter.next() {
                    if len >= inline_capacity {
                        let heap_capacity = inline_capacity + iter.size_hint().1.unwrap_or(0);
                        let mut vec = ThinVec::with_capacity(heap_capacity);

                        vec.extend(array.into_iter());
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;
    use std::num::NonZeroUsize;

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

    #[test]
    fn insert1() {
        let mut v: Vekk<[char; 4]> = Default::default();
        v.insert(0, 'a');
        assert_eq!(v.as_slice(), &['a']);
    }

    #[test]
    fn insert2() {
        let mut v: Vekk<[char; 4]> = vec!['a', 'c'].into();
        v.insert(1, 'b');
        assert_eq!(v.as_slice(), &['a', 'b', 'c']);
    }

    #[test]
    fn insert3() {
        let mut v: Vekk<[char; 4]> = vec!['a', 'b'].into();
        v.insert(2, 'c');
        assert_eq!(v.as_slice(), &['a', 'b', 'c']);
    }

    #[test]
    fn insert4() {
        let mut v: Vekk<[char; 4]> = vec!['a', 'b', 'd', 'e'].into();
        assert!(matches!(v.repr, Repr::Inline { .. }));
        v.insert(2, 'c');
        assert_eq!(v.as_slice(), &['a', 'b', 'c', 'd', 'e']);
    }

    #[test]
    fn insert_extend() {
        let mut v: Vekk<[char; 4]> = Default::default();

        v.insert(0, 'b');
        assert_eq!(v.as_slice(), &['b']);

        v.extend(['d']);
        assert_eq!(v.as_slice(), &['b', 'd']);

        v.insert(1, 'c');
        assert_eq!(v.as_slice(), &['b', 'c', 'd']);
        assert!(matches!(v.repr, Repr::Inline { .. }));

        v.insert(3, 'e');
        assert_eq!(v.as_slice(), &['b', 'c', 'd', 'e']);
        assert!(matches!(v.repr, Repr::Inline { .. }));

        v.insert(0, 'a');
        assert_eq!(v.as_slice(), &['a', 'b', 'c', 'd', 'e']);
        assert!(matches!(v.repr, Repr::Heap(_)));
    }

    #[allow(unused)]
    enum Test<T> {
        A(u16, T),
        B(NonZeroUsize),
    }

    #[test]
    fn test_size() {
        assert_eq!(16, size_of::<Test<u64>>());
        assert_eq!(16, size_of::<Test<u32>>());
        assert_eq!(16, size_of::<Test<u16>>());
        assert_eq!(16, size_of::<Test<u8>>());
        assert_eq!(16, size_of::<Test<()>>());
    }
}
