// Copyright (C) 2019-2022 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]

pub mod add_checked;
pub mod add_wrapped;
pub mod and;
pub mod compare;
pub mod div_checked;
pub mod div_wrapped;
pub mod equal;
pub mod from_bits;
pub mod msb;
pub mod mul_checked;
pub mod mul_wrapped;
pub mod neg;
pub mod not;
pub mod one;
pub mod or;
pub mod pow_checked;
pub mod pow_wrapped;
pub mod shl_checked;
pub mod shl_wrapped;
pub mod shr_checked;
pub mod shr_wrapped;
pub mod sub_checked;
pub mod sub_wrapped;
pub mod ternary;
pub mod to_bits;
pub mod to_field;
pub mod to_fields;
pub mod xor;
pub mod zero;

pub type I8<E> = Integer<E, i8>;
pub type I16<E> = Integer<E, i16>;
pub type I32<E> = Integer<E, i32>;
pub type I64<E> = Integer<E, i64>;
pub type I128<E> = Integer<E, i128>;

pub type U8<E> = Integer<E, u8>;
pub type U16<E> = Integer<E, u16>;
pub type U32<E> = Integer<E, u32>;
pub type U64<E> = Integer<E, u64>;
pub type U128<E> = Integer<E, u128>;

#[cfg(test)]
use snarkvm_circuits_environment::assert_scope;

use snarkvm_circuits_environment::prelude::*;
use snarkvm_circuits_types_boolean::Boolean;
use snarkvm_circuits_types_field::Field;

use core::marker::PhantomData;

#[derive(Clone)]
pub struct Integer<E: Environment, I: IntegerType> {
    bits_le: Vec<Boolean<E>>,
    phantom: PhantomData<I>,
}

impl<E: Environment, I: IntegerType> IntegerTrait<Boolean<E>, I, U8<E>, U16<E>, U32<E>> for Integer<E, I> {}

impl<E: Environment, I: IntegerType> IntegerCore<Boolean<E>, I> for Integer<E, I> {}

impl<E: Environment, I: IntegerType> DataType<Boolean<E>> for Integer<E, I> {}

impl<E: Environment, I: IntegerType> Inject for Integer<E, I> {
    type Primitive = I;

    /// Initializes a new integer.
    fn new(mode: Mode, value: Self::Primitive) -> Self {
        let mut bits_le = Vec::with_capacity(I::BITS);
        let mut value = value.to_le();
        for _ in 0..I::BITS {
            bits_le.push(Boolean::new(mode, value & I::one() == I::one()));
            value = value.wrapping_shr(1u32);
        }
        Self::from_bits_le(&bits_le)
    }
}

// TODO (@pranav) Document
impl<E: Environment, I: IntegerType> Integer<E, I> {
    fn cast_as_dual(self) -> Integer<E, I::Dual> {
        Integer::<E, I::Dual> { bits_le: self.bits_le, phantom: Default::default() }
    }
}

impl<E: Environment, I: IntegerType> Eject for Integer<E, I> {
    type Primitive = I;

    ///
    /// Ejects the mode of the integer.
    ///
    fn eject_mode(&self) -> Mode {
        E::eject_mode(&self.bits_le)
    }

    ///
    /// Ejects the integer as a constant integer value.
    ///
    fn eject_value(&self) -> Self::Primitive {
        self.bits_le.iter().rev().fold(I::zero(), |value, bit| match bit.eject_value() {
            true => (value.wrapping_shl(1)) ^ I::one(),
            false => (value.wrapping_shl(1)) ^ I::zero(),
        })
    }
}

impl<E: Environment, I: IntegerType> Parser for Integer<E, I> {
    type Environment = E;

    /// Parses a string into an integer circuit.
    #[inline]
    fn parse(string: &str) -> ParserResult<Self> {
        // Parse the negative sign '-' from the string.
        let (string, negation) = map(opt(tag("-")), |neg: Option<&str>| neg.unwrap_or_default().to_string())(string)?;
        // Parse the digits from the string.
        let (string, primitive) = recognize(many1(terminated(one_of("0123456789"), many0(char('_')))))(string)?;
        // Combine the sign and primitive.
        let primitive = negation + primitive;
        // Parse the value from the string.
        let (string, value) = map_res(tag(Self::type_name()), |_| primitive.replace('_', "").parse())(string)?;
        // Parse the mode from the string.
        let (string, mode) = opt(pair(tag("."), Mode::parse))(string)?;

        match mode {
            Some((_, mode)) => Ok((string, Self::new(mode, value))),
            None => Ok((string, Self::new(Mode::Constant, value))),
        }
    }
}

impl<E: Environment, I: IntegerType> TypeName for Integer<E, I> {
    /// Returns the type name of the circuit as a string.
    #[inline]
    fn type_name() -> &'static str {
        I::type_name()
    }
}

impl<E: Environment, I: IntegerType> Debug for Integer<E, I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.eject_value())
    }
}

impl<E: Environment, I: IntegerType> Display for Integer<E, I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}.{}", self.eject_value(), Self::type_name(), self.eject_mode())
    }
}

impl<E: Environment, I: IntegerType> From<Integer<E, I>> for LinearCombination<E::BaseField> {
    fn from(integer: Integer<E, I>) -> Self {
        From::from(&integer)
    }
}

impl<E: Environment, I: IntegerType> From<&Integer<E, I>> for LinearCombination<E::BaseField> {
    fn from(integer: &Integer<E, I>) -> Self {
        // Reconstruct the bits as a linear combination representing the original field value.
        let mut accumulator = E::zero();
        let mut coefficient = E::BaseField::one();
        for bit in &integer.bits_le {
            accumulator += LinearCombination::from(bit) * coefficient;
            coefficient = coefficient.double();
        }
        accumulator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_circuits_environment::Circuit;
    use snarkvm_utilities::{test_rng, UniformRand};

    const ITERATIONS: usize = 1000;

    fn check_new<I: IntegerType>(mode: Mode) {
        let expected: I = UniformRand::rand(&mut test_rng());
        let candidate = Integer::<Circuit, I>::new(mode, expected);
        assert_eq!(mode.is_constant(), candidate.is_constant());
        assert_eq!(candidate.eject_value(), expected);
    }

    fn check_min_max<I: IntegerType>(mode: Mode) {
        assert_eq!(I::MIN, Integer::<Circuit, I>::new(mode, I::MIN).eject_value());
        assert_eq!(I::MAX, Integer::<Circuit, I>::new(mode, I::MAX).eject_value());
    }

    fn check_parser<I: IntegerType>() {
        for mode in [Mode::Constant, Mode::Public, Mode::Private] {
            for _ in 0..ITERATIONS {
                let value: I = UniformRand::rand(&mut test_rng());
                let expected = Integer::<Circuit, I>::new(mode, value);

                let (_, candidate) = Integer::<Circuit, I>::parse(&format!("{expected}")).unwrap();
                assert_eq!(mode, candidate.eject_mode());
                assert_eq!(value, candidate.eject_value());
                assert_eq!(expected.eject_mode(), candidate.eject_mode());
                assert_eq!(expected.eject_value(), candidate.eject_value());
            }
        }
    }

    fn check_debug<I: IntegerType>() {
        // Constant
        let candidate = Integer::<Circuit, I>::new(Mode::Constant, I::one() + I::one());
        assert_eq!("2", format!("{:?}", candidate));

        // Public
        let candidate = Integer::<Circuit, I>::new(Mode::Public, I::one() + I::one());
        assert_eq!("2", format!("{:?}", candidate));

        // Private
        let candidate = Integer::<Circuit, I>::new(Mode::Private, I::one() + I::one());
        assert_eq!("2", format!("{:?}", candidate));
    }

    fn check_display<I: IntegerType>() {
        // Constant
        let candidate = Integer::<Circuit, I>::new(Mode::Constant, I::one() + I::one());
        assert_eq!(format!("2{}.constant", I::type_name()), format!("{}", candidate));

        // Public
        let candidate = Integer::<Circuit, I>::new(Mode::Public, I::one() + I::one());
        assert_eq!(format!("2{}.public", I::type_name()), format!("{}", candidate));

        // Private
        let candidate = Integer::<Circuit, I>::new(Mode::Private, I::one() + I::one());
        assert_eq!(format!("2{}.private", I::type_name()), format!("{}", candidate));
    }

    fn run_test<I: IntegerType>() {
        for _ in 0..ITERATIONS {
            check_new::<I>(Mode::Constant);
            check_new::<I>(Mode::Public);
            check_new::<I>(Mode::Private);
        }

        check_min_max::<I>(Mode::Constant);
        check_min_max::<I>(Mode::Public);
        check_min_max::<I>(Mode::Private);

        check_parser::<I>();
        check_debug::<I>();
        check_display::<I>();
    }

    #[test]
    fn test_i8() {
        run_test::<i8>();
    }

    #[test]
    fn test_i16() {
        run_test::<i16>();
    }

    #[test]
    fn test_i32() {
        run_test::<i32>();
    }

    #[test]
    fn test_i64() {
        run_test::<i64>();
    }

    #[test]
    fn test_i128() {
        run_test::<i128>();
    }

    #[test]
    fn test_u8() {
        run_test::<u8>();
    }

    #[test]
    fn test_u16() {
        run_test::<u16>();
    }

    #[test]
    fn test_u32() {
        run_test::<u32>();
    }

    #[test]
    fn test_u64() {
        run_test::<u64>();
    }

    #[test]
    fn test_u128() {
        run_test::<u128>();
    }
}

#[cfg(test)]
mod test_utilities {
    use core::{
        fmt::{Debug, Display},
        panic::UnwindSafe,
    };
    use snarkvm_circuits_environment::{assert_scope, assert_scope_fails, Circuit, Eject, Environment};

    pub fn check_operation_passes<V: Debug + Display + PartialEq, LHS, RHS, OUT: Eject<Primitive = V>>(
        name: &str,
        case: &str,
        expected: V,
        a: LHS,
        b: RHS,
        operation: impl FnOnce(LHS, RHS) -> OUT,
        num_constants: usize,
        num_public: usize,
        num_private: usize,
        num_constraints: usize,
    ) {
        Circuit::scope(name, || {
            let candidate = operation(a, b);
            assert_eq!(expected, candidate.eject_value(), "{} != {} := {}", expected, candidate.eject_value(), case);
            assert_scope!(case, num_constants, num_public, num_private, num_constraints);
        });
        Circuit::reset();
    }

    pub fn check_operation_passes_without_counts<
        V: Debug + Display + PartialEq,
        LHS,
        RHS,
        OUT: Eject<Primitive = V>,
    >(
        name: &str,
        case: &str,
        expected: V,
        a: LHS,
        b: RHS,
        operation: impl FnOnce(LHS, RHS) -> OUT,
    ) {
        Circuit::scope(name, || {
            let candidate = operation(a, b);
            assert_eq!(expected, candidate.eject_value(), "{} != {} := {}", expected, candidate.eject_value(), case);
        });
        Circuit::reset();
    }

    pub fn check_operation_fails<LHS, RHS, OUT>(
        name: &str,
        case: &str,
        a: LHS,
        b: RHS,
        operation: impl FnOnce(LHS, RHS) -> OUT,
        num_constants: usize,
        num_public: usize,
        num_private: usize,
        num_constraints: usize,
    ) {
        Circuit::scope(name, || {
            let _candidate = operation(a, b);
            assert_scope_fails!(case, num_constants, num_public, num_private, num_constraints);
        });
        Circuit::reset();
    }

    pub fn check_operation_fails_without_counts<LHS, RHS, OUT>(
        name: &str,
        case: &str,
        a: LHS,
        b: RHS,
        operation: impl FnOnce(LHS, RHS) -> OUT,
    ) {
        Circuit::scope(name, || {
            let _candidate = operation(a, b);
            assert!(!Circuit::is_satisfied(), "{} (!is_satisfied)", case);
        });
        Circuit::reset();
    }

    pub fn check_operation_halts<LHS: UnwindSafe, RHS: UnwindSafe, OUT>(
        a: LHS,
        b: RHS,
        operation: impl FnOnce(LHS, RHS) -> OUT + UnwindSafe,
    ) {
        let result = std::panic::catch_unwind(|| operation(a, b));
        assert!(result.is_err());
    }

    pub fn check_unary_operation_passes<V: Debug + Display + PartialEq, IN, OUT: Eject<Primitive = V>>(
        name: &str,
        case: &str,
        expected: V,
        input: IN,
        operation: impl FnOnce(IN) -> OUT,
        num_constants: usize,
        num_public: usize,
        num_private: usize,
        num_constraints: usize,
    ) {
        Circuit::scope(name, || {
            let candidate = operation(input);
            assert_eq!(expected, candidate.eject_value(), "{}", case);
            assert_scope!(case, num_constants, num_public, num_private, num_constraints);
        });
        Circuit::reset();
    }

    pub fn check_unary_operation_fails<IN, OUT>(
        name: &str,
        case: &str,
        input: IN,
        operation: impl FnOnce(IN) -> OUT,
        num_constants: usize,
        num_public: usize,
        num_private: usize,
        num_constraints: usize,
    ) {
        Circuit::scope(name, || {
            let _candidate = operation(input);
            assert_scope_fails!(case, num_constants, num_public, num_private, num_constraints);
        });
        Circuit::reset();
    }
}