//! What an app, or a library writing a view of its own, imports: the views, the traits their
//! members come from, and the values they take.

pub use crate::{
  Alignment, Angle, Apply, Arrangement, AsPart, AsPartBase, AxisAlignment, Border, Bubbling,
  Button, ButtonBase, Canvas, Checkbox, CheckboxBase, ChildOf, ChoiceBase, ClipboardOp, Color,
  Column, ColumnBase, Container, ContainerBase, ContextMenu, ContextMenuBase, Control, ControlBase,
  ControlMembersBase, Corners, Cx, DecorationBase, Definitions, Derived, DerivedBase, Dimension,
  Edges, EditingBase, Effect, Environment, Fill, Flexibility, Flexible, FlexibleBase, Flow,
  FontWeight, Grid, GridBase, GridCell, GridCellBase, GridPlacement, GridTrack, Image, ImageBase,
  Inset, Keyboard, Label, LabelPlacement, LinearGradient, Listbox, ListboxBase, ListboxItem,
  ListboxItemBase, Media, Memo, MenuEntry, MenuItem, MenuItemBase, MotionBase, Node, NodeHandle,
  Offset, OptionsBase, Overlay, OverlayBase, Paint, Painted, PaintedBase, Painting, Part, Picker,
  PickerBase, PickerItem, PickerItemBase, Pinned, PinnedBase, Platform, RadialGradient, Radio,
  RadioBase, RadioGroup, RadioGroupBase, Resource, Ring, Row, RowBase, RunBase, RunWithEntries,
  Scheme, Scroll, ScrollAxes, ScrollBase, Shadow, Shortcut, Side, Signal, Slider, SliderBase,
  Spacing, Stack, StackBase, State, SweepGradient, SystemColors, Text, TextBase, TextEditor,
  TextEditorBase, TextField, TextFieldBase, TextSelection, Theme, ThemeBase, Toggle, ToggleBase,
  Token, TokenRef, TokenSet, Tracks, UnitPoint, Value, View, When as _, media, platform,
};
/// The foundation's view trait, unnamed: what a view's `build` and `into_any` are called
/// through.
pub use skene_foundation::view::View as _;
