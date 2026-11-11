package app.ghostleaf;

import android.app.NativeActivity;
import android.content.ClipData;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;

import java.util.ArrayList;

/**
 * Thin Java shell around the native (Rust) app.
 *
 * Everything is drawn by libghostleaf.so; this class only does what needs the
 * Android framework: the system file picker, "Open with Ghostleaf", reading a
 * picked document without copying it (a file descriptor), and colouring the
 * system bars. Rust calls these methods over JNI (app/src/android.rs) and polls
 * {@link #takeDocuments()}; there are no native callbacks.
 *
 * Privacy: no network, no copies. Picked files stay where they are; Ghostleaf
 * keeps a read permission for them so they can be reopened after a restart.
 */
public class MainActivity extends NativeActivity {
    private static final int PICK_DOCUMENTS = 4711;
    

    /** "uri\nname" entries waiting for the native side. */
    private final ArrayList<String> pending = new ArrayList<>();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Window w = getWindow();
        // Lay out below the status bar and above the navigation bar, and let us colour them.
        w.clearFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
                | WindowManager.LayoutParams.FLAG_TRANSLUCENT_STATUS
                | WindowManager.LayoutParams.FLAG_TRANSLUCENT_NAVIGATION);
        w.addFlags(WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS);
        if (Build.VERSION.SDK_INT >= 30) {
            w.setDecorFitsSystemWindows(true);
        }
        handleViewIntent(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleViewIntent(intent);
    }

    /** "Open with Ghostleaf" from a file manager, browser download list, etc. */
    private void handleViewIntent(Intent intent) {
        if (intent != null && Intent.ACTION_VIEW.equals(intent.getAction()) && intent.getData() != null) {
            add(intent.getData(), (intent.getFlags() & Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0);
        }
    }

    /** Called from Rust: show the system document picker. */
    public void pickDocuments() {
        runOnUiThread(() -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            // all files: some providers label .txt/.csv with odd MIME types, which
            // would grey them out; Ghostleaf filters by file name instead
            i.setType("*/*");
            i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            try {
                startActivityForResult(i, PICK_DOCUMENTS);
            } catch (Exception ignored) {
                // no document provider available
            }
        });
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != PICK_DOCUMENTS || resultCode != RESULT_OK || data == null) {
            return;
        }
        ClipData clip = data.getClipData();
        if (clip != null) {
            for (int k = 0; k < clip.getItemCount(); k++) {
                add(clip.getItemAt(k).getUri(), true);
            }
        } else if (data.getData() != null) {
            add(data.getData(), true);
        }
    }

    private void add(Uri uri, boolean persist) {
        if (uri == null) {
            return;
        }
        if (persist) {
            try {
                // keep read access across restarts so the book can be reopened from the library
                getContentResolver().takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (Exception ignored) {
                // not persistable (e.g. some "Open with" sources): still readable for this session
            }
        }
        synchronized (pending) {
            pending.add(uri.toString() + "\n" + displayName(uri));
        }
    }

    private String displayName(Uri uri) {
        String name = null;
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                name = c.getString(0);
            }
        } catch (Exception ignored) {
            // fall back below
        }
        if (name == null) {
            name = uri.getLastPathSegment();
        }
        return name == null ? "document" : name.replace('\n', ' ');
    }

    /** Called from Rust: documents picked or opened since the last call ("uri\nname"). */
    public String[] takeDocuments() {
        synchronized (pending) {
            String[] out = pending.toArray(new String[0]);
            pending.clear();
            return out;
        }
    }

    /** Called from Rust: a readable file descriptor for a document, or -1. The caller owns it. */
    public int openFd(String uri) {
        try {
            ParcelFileDescriptor pfd = getContentResolver().openFileDescriptor(Uri.parse(uri), "r");
            return pfd == null ? -1 : pfd.detachFd();
        } catch (Exception e) {
            return -1;
        }
    }

    /** Called from Rust: colour the status and navigation bars to match the reading theme. */
    @SuppressWarnings("deprecation")
    public void setBarColors(final int argb, final boolean lightBackground) {
        runOnUiThread(() -> {
            Window w = getWindow();
            w.setStatusBarColor(argb);
            w.setNavigationBarColor(argb);
            View decor = w.getDecorView();
            int flags = decor.getSystemUiVisibility();
            int light = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR;
            if (Build.VERSION.SDK_INT >= 26) {
                light |= View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            }
            decor.setSystemUiVisibility(lightBackground ? (flags | light) : (flags & ~light));
        });
    }
}
