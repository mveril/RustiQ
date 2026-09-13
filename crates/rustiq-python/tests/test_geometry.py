import math

import pytest

from rustiq import Atom, Geometry, Molecule


def test_atom_accepts_symbol_or_atomic_number():
    hydrogen = Atom("H", 1.0, 0.0, 0.0)
    helium = Atom(2, 0.0, 1.0, 0.0)

    assert hydrogen.element == "H"
    assert hydrogen.coordinates == (1.0, 0.0, 0.0)
    assert helium.element == "He"


def test_geometry_accepts_an_iterable_of_atom_tuples():
    geometry = Geometry((("H", x, 0.0, 0.0) for x in (-1.0, 1.0)), "H2")

    assert len(geometry) == 2
    assert geometry.comment == "H2"
    assert geometry.coordinates() == [(-1.0, 0.0, 0.0), (1.0, 0.0, 0.0)]


def test_geometry_manipulation_methods_accept_three_coordinate_tuples():
    geometry = Geometry([("H", 1.0, 0.0, 0.0)])

    geometry.translate((0.0, 1.0, 0.0))
    geometry.rotate((0.0, 0.0, 1.0), math.pi / 2)
    geometry.centering()

    assert geometry.coordinates()[0] == pytest.approx((0.0, 0.0, 0.0))


def test_geometry_rejects_invalid_axis():
    with pytest.raises(ValueError, match="zero vector"):
        Geometry([("H", 0.0, 0.0, 0.0)]).rotate((0.0, 0.0, 0.0), 1.0)

    with pytest.raises(ValueError, match="tuple"):
        Geometry([("H", 0.0, 0.0, 0.0)]).rotate([0.0, 0.0, 1.0], 1.0)


def test_molecule_defaults_and_validates_with_try_new():
    molecule = Molecule([("He", 0.0, 0.0, 0.0)])

    assert molecule.charge == 0
    assert molecule.multiplicity == 1
    assert molecule.geometry.comment == ""

    with pytest.raises(ValueError, match="electron configuration"):
        Molecule([("H", 0.0, 0.0, 0.0)])
