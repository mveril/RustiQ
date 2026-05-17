pub (crate) enum AtomParseError{
  LineShouldHaveForPart,
  AtomCoordinateError(usize,ParseFloatError)

}

impl Error for AtomParseError{
  
}