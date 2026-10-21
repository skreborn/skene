#include <gtk/gtk.h>
#include <stdio.h>

static GtkWidget *the_popover;

static gboolean on_key(GtkEventControllerKey *c, guint keyval, guint keycode, GdkModifierType state, gpointer where) {
  if (keyval == GDK_KEY_F2 && the_popover) {
    gtk_popover_popup(GTK_POPOVER(the_popover));
    printf("POPUP\n");
  }
  printf("KEY %s keyval=%s state=%#x control=%d\n", (const char *)where, gdk_keyval_name(keyval), state, (state & GDK_CONTROL_MASK) != 0);
  fflush(stdout);
  return FALSE;
}

static gboolean on_shortcut(GtkWidget *w, GVariant *args, gpointer data) {
  printf("SHORTCUT ctrl+r\n");
  fflush(stdout);
  return TRUE;
}

static void on_clicked(GtkButton *button, gpointer popover) {
  gtk_popover_popup(GTK_POPOVER(popover));
  printf("POPUP\n");
  fflush(stdout);
}

static void activate(GtkApplication *app, gpointer data) {
  GtkWidget *window = gtk_application_window_new(app);
  GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *button = gtk_button_new_with_label("Open");
  GtkWidget *popover = gtk_popover_new();
  GtkWidget *inner = gtk_button_new_with_label("Inside");
  GtkEventController *key = gtk_event_controller_key_new();
  GtkEventController *inner_key = gtk_event_controller_key_new();
  GtkEventController *shortcuts = gtk_shortcut_controller_new();

  the_popover = popover;
  gtk_popover_set_child(GTK_POPOVER(popover), inner);
  gtk_popover_set_autohide(GTK_POPOVER(popover), TRUE);
  gtk_widget_set_parent(popover, button);
  g_signal_connect(button, "clicked", G_CALLBACK(on_clicked), popover);
  gtk_event_controller_set_propagation_phase(key, GTK_PHASE_CAPTURE);
  g_signal_connect(key, "key-pressed", G_CALLBACK(on_key), "window");
  gtk_widget_add_controller(window, key);
  gtk_event_controller_set_propagation_phase(inner_key, GTK_PHASE_CAPTURE);
  g_signal_connect(inner_key, "key-pressed", G_CALLBACK(on_key), "popover");
  gtk_widget_add_controller(popover, inner_key);
  gtk_shortcut_controller_set_scope(GTK_SHORTCUT_CONTROLLER(shortcuts), GTK_SHORTCUT_SCOPE_GLOBAL);
  gtk_shortcut_controller_add_shortcut(GTK_SHORTCUT_CONTROLLER(shortcuts),
    gtk_shortcut_new(gtk_shortcut_trigger_parse_string("<Control>r"), gtk_callback_action_new(on_shortcut, NULL, NULL)));
  gtk_widget_add_controller(window, shortcuts);
  gtk_box_append(GTK_BOX(box), button);
  gtk_window_set_child(GTK_WINDOW(window), box);
  gtk_window_set_default_size(GTK_WINDOW(window), 300, 200);
  gtk_window_present(GTK_WINDOW(window));
  gtk_widget_grab_focus(button);
}

int main(int argc, char **argv) {
  GtkApplication *app = gtk_application_new("dev.skene.Mods", G_APPLICATION_NON_UNIQUE);
  g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
  return g_application_run(G_APPLICATION(app), argc, argv);
}
