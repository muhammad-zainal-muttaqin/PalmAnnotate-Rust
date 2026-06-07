package dev.sawitulm.palmannotate.rust.nativebridge

import android.Manifest
import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageFormat
import android.graphics.Rect
import android.graphics.YuvImage
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.net.Uri
import android.os.Build
import android.util.Base64
import android.util.Log
import android.util.Size
import androidx.activity.result.ActivityResult
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.ImageProxy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.documentfile.provider.DocumentFile
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.orbbec.obsensor.ColorFrame
import com.orbbec.obsensor.Config
import com.orbbec.obsensor.DepthFrame
import com.orbbec.obsensor.Device
import com.orbbec.obsensor.DeviceChangedCallback
import com.orbbec.obsensor.DeviceList
import com.orbbec.obsensor.FrameSet
import com.orbbec.obsensor.OBContext
import com.orbbec.obsensor.Pipeline
import com.orbbec.obsensor.StreamProfileList
import com.orbbec.obsensor.VideoStreamProfile
import com.orbbec.obsensor.types.Format
import com.orbbec.obsensor.types.AlignMode
import com.orbbec.obsensor.types.FrameAggregateOutputMode
import com.orbbec.obsensor.types.LogSeverity
import com.orbbec.obsensor.types.SensorType
import com.orbbec.obsensor.types.StreamType
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

@InvokeArg
class TreeArgs {
    var treeUri: String? = null
}

@InvokeArg
class SafPathArgs {
    var treeUri: String? = null
    var relativePath: String? = null
}

@InvokeArg
class SafCopyArgs {
    var treeUri: String? = null
    var relativePath: String? = null
    var sourcePath: String? = null
    var mimeType: String? = null
}

@InvokeArg
class TempPathArgs {
    var path: String? = null
}

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.CAMERA], alias = "camera")
    ]
)
class PalmNativePlugin(private val hostActivity: Activity) : Plugin(hostActivity) {
    companion object {
        private const val ORBBEC_VENDOR_ID = 0x2BC5
        private const val USB_PERMISSION = "dev.sawitulm.palmannotate.rust.USB_PERMISSION"
        private const val TAG = "PalmAnnotateOrbbec"
        private const val JPEG_QUALITY = 88
        private const val FRAME_TIMEOUT_MS = 1_500L
        private const val DEVICE_QUERY_RETRIES = 8
        private const val DEVICE_QUERY_DELAY_MS = 250L
        private const val CAPTURE_VIA_PUMP_TIMEOUT_MS = 6_000L
    }

    private class CaptureWaiter {
        @Volatile var result: CapturedRgbd? = null
        @Volatile var error: Exception? = null
        private val latch = CountDownLatch(1)

        fun resolve(value: CapturedRgbd) {
            result = value
            latch.countDown()
        }

        fun reject(value: Exception) {
            error = value
            latch.countDown()
        }

        fun await(): CapturedRgbd {
            if (!latch.await(CAPTURE_VIA_PUMP_TIMEOUT_MS, TimeUnit.MILLISECONDS)) {
                throw IllegalStateException("Timed out waiting for Orbbec frame")
            }
            error?.let { throw it }
            return result ?: throw IllegalStateException("No Orbbec frame produced")
        }
    }

    private data class CapturedJpeg(
        val bytes: ByteArray,
        val width: Int,
        val height: Int,
        val sourceFormat: String
    )

    private data class CapturedDepth(
        val bytes: ByteArray,
        val width: Int,
        val height: Int,
        val sourceFormat: String,
        val valueScale: Float
    )

    private data class CapturedRgbd(
        val color: CapturedJpeg,
        val depth: CapturedDepth?
    )

    private val worker: ExecutorService = Executors.newSingleThreadExecutor()
    private val orbbecLock = Any()
    private var cameraProvider: ProcessCameraProvider? = null
    private var imageCapture: ImageCapture? = null
    private var cameraAnalysis: ImageAnalysis? = null
    @Volatile private var lastCameraPreviewAt = 0L
    private var obContext: OBContext? = null
    private var obDevice: Device? = null
    private var obPipeline: Pipeline? = null
    private var obDepth = false
    @Volatile private var orbbecPreviewRunning = false
    private var selectedUid: String? = null
    private var streamPump: Thread? = null
    private val pendingCapture = AtomicReference<CaptureWaiter?>(null)
    private val deviceChangedCallback = object : DeviceChangedCallback {
        override fun onDeviceAttach(deviceList: DeviceList) {
            val count = try {
                deviceList.deviceCount
            } catch (_: Exception) {
                0
            } finally {
                safeClose(deviceList)
            }
            trigger(
                "orbbec-device-change",
                JSObject().put("attached", true).put("count", count)
            )
        }

        override fun onDeviceDetach(deviceList: DeviceList) {
            var selectedDetached = false
            try {
                val uid = synchronized(orbbecLock) { selectedUid }
                if (uid != null) {
                    for (index in 0 until deviceList.deviceCount) {
                        if (uid == deviceList.getUid(index)) {
                            selectedDetached = true
                            break
                        }
                    }
                }
            } catch (_: Exception) {
                selectedDetached = true
            } finally {
                safeClose(deviceList)
            }
            if (selectedDetached) {
                stopOrbbecPreview()
                worker.execute {
                    joinOrbbecPreview()
                    synchronized(orbbecLock) { closeOrbbecLocked() }
                }
            }
            trigger(
                "orbbec-device-change",
                JSObject().put("attached", false).put("count", 0)
            )
        }
    }

    @Command
    fun camera_status(invoke: Invoke) {
        invoke.resolve(
            JSObject()
                .put("available", hostActivity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY))
                .put("permission", hasCameraPermission())
                .put("opened", imageCapture != null)
        )
    }

    @Command
    fun camera_start(invoke: Invoke) {
        if (!hasCameraPermission()) {
            requestPermissionForAlias("camera", invoke, "cameraPermissionResult")
            return
        }
        openCamera(invoke)
    }

    @PermissionCallback
    private fun cameraPermissionResult(invoke: Invoke) {
        if (hasCameraPermission()) openCamera(invoke)
        else invoke.reject("Camera permission was denied")
    }

    private fun openCamera(invoke: Invoke) {
        val future = ProcessCameraProvider.getInstance(hostActivity)
        future.addListener({
            try {
                val provider = future.get()
                val capture = ImageCapture.Builder()
                    .setCaptureMode(ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY)
                    .build()
                val analysis = ImageAnalysis.Builder()
                    .setResolutionSelector(
                        ResolutionSelector.Builder()
                            .setResolutionStrategy(
                                ResolutionStrategy(
                                    Size(640, 480),
                                    ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER
                                )
                            )
                            .build()
                    )
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(worker) { frame -> emitCameraPreview(frame) }
                provider.unbindAll()
                provider.bindToLifecycle(
                    hostActivity as androidx.lifecycle.LifecycleOwner,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    capture,
                    analysis
                )
                cameraProvider = provider
                imageCapture = capture
                cameraAnalysis = analysis
                invoke.resolve(JSObject().put("opened", true).put("previewMode", "event"))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "CameraX failed to open")
            }
        }, ContextCompat.getMainExecutor(hostActivity))
    }

    @Command
    fun camera_capture(invoke: Invoke) {
        val capture = imageCapture
        if (capture == null) {
            invoke.reject("Camera is not open")
            return
        }
        val file = tempFile("camerax", ".jpg")
        val options = ImageCapture.OutputFileOptions.Builder(file).build()
        capture.takePicture(
            options,
            worker,
            object : ImageCapture.OnImageSavedCallback {
                override fun onImageSaved(result: ImageCapture.OutputFileResults) {
                    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    BitmapFactory.decodeFile(file.absolutePath, bounds)
                    invoke.resolve(
                        JSObject()
                            .put("path", file.absolutePath)
                            .put("format", "jpeg")
                            .put("source", "camerax")
                            .put("width", bounds.outWidth)
                            .put("height", bounds.outHeight)
                    )
                }

                override fun onError(error: ImageCaptureException) {
                    file.delete()
                    invoke.reject(error.message ?: "Camera capture failed")
                }
            }
        )
    }

    @Command
    fun camera_stop(invoke: Invoke) {
        cameraProvider?.unbindAll()
        cameraProvider = null
        imageCapture = null
        cameraAnalysis = null
        invoke.resolve(JSObject().put("closed", true))
    }

    @Command
    fun temp_delete(invoke: Invoke) {
        try {
            val requested = File(required(invoke.parseArgs(TempPathArgs::class.java).path, "path"))
                .canonicalFile
            val cache = File(hostActivity.cacheDir, "palmannotate").canonicalFile
            require(requested.path.startsWith(cache.path + File.separator)) {
                "Temporary path is outside PalmAnnotate cache"
            }
            invoke.resolve(JSObject().put("removed", !requested.exists() || requested.delete()))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Temporary file cleanup failed")
        }
    }

    @Command
    fun orbbec_status(invoke: Invoke) {
        val devices = orbbecDevices()
        invoke.resolve(
            JSObject()
                .put("available", devices.isNotEmpty())
                .put("opened", obPipeline != null)
                .put("count", devices.size)
        )
    }

    @Command
    fun orbbec_list(invoke: Invoke) {
        val manager = usbManager()
        val list = JSArray()
        orbbecDevices().forEach { device ->
            list.put(
                JSObject()
                    .put("name", device.productName ?: device.deviceName)
                    .put("vendorId", device.vendorId)
                    .put("productId", device.productId)
                    .put("deviceName", device.deviceName)
                    .put("hasPermission", manager?.hasPermission(device) == true)
            )
        }
        invoke.resolve(JSObject().put("devices", list))
    }

    @Command
    fun orbbec_request_permission(invoke: Invoke) {
        val manager = usbManager()
        val device = orbbecDevices().firstOrNull()
        if (manager == null || device == null) {
            invoke.reject("No Orbbec device found")
            return
        }
        if (manager.hasPermission(device)) {
            invoke.resolve(JSObject().put("granted", true))
            return
        }
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                if (intent.action != USB_PERMISSION) return
                try { context.unregisterReceiver(this) } catch (_: Exception) {}
                invoke.resolve(
                    JSObject().put(
                        "granted",
                        intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                    )
                )
            }
        }
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
        val pending = PendingIntent.getBroadcast(
            hostActivity,
            0,
            Intent(USB_PERMISSION).setPackage(hostActivity.packageName),
            flags
        )
        val filter = IntentFilter(USB_PERMISSION)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            hostActivity.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            hostActivity.registerReceiver(receiver, filter)
        }
        manager.requestPermission(device, pending)
    }

    @Command
    fun orbbec_open(invoke: Invoke) {
        worker.execute {
            try {
                joinOrbbecPreview()
                val result = synchronized(orbbecLock) { openOrbbecLocked() }
                startOrbbecPreview()
                invoke.resolve(result)
            } catch (error: Exception) {
                stopOrbbecPreview()
                joinOrbbecPreview()
                synchronized(orbbecLock) { closeOrbbecLocked() }
                invoke.reject(error.message ?: "Failed to open Orbbec camera")
            }
        }
    }

    @Command
    fun orbbec_capture(invoke: Invoke) {
        worker.execute {
            try {
                synchronized(orbbecLock) { openOrbbecLocked() }
                val frame = if (orbbecPreviewRunning) {
                    captureViaPump()
                } else {
                    joinOrbbecPreview()
                    captureOrbbecDirect()
                }
                invoke.resolve(writeCapturedFrame(frame))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Orbbec capture failed")
            }
        }
    }

    @Command
    fun orbbec_close(invoke: Invoke) {
        stopOrbbecPreview()
        worker.execute {
            joinOrbbecPreview()
            synchronized(orbbecLock) { closeOrbbecLocked() }
            invoke.resolve(JSObject().put("closed", true))
        }
    }

    @Command
    fun orbbec_refresh(invoke: Invoke) {
        stopOrbbecPreview()
        worker.execute {
            joinOrbbecPreview()
            synchronized(orbbecLock) { closeOrbbecLocked() }
            val devices = orbbecDevices()
            invoke.resolve(
                JSObject()
                    .put("available", devices.isNotEmpty())
                    .put("count", devices.size)
            )
        }
    }

    @Command
    fun saf_pick_folder(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                    Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
            )
        }
        startActivityForResult(invoke, intent, "safPickResult")
    }

    @Command
    fun saf_pick_json(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "application/json"
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        startActivityForResult(invoke, intent, "safJsonResult")
    }

    @ActivityCallback
    private fun safPickResult(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().put("cancelled", true))
            return
        }
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        try { hostActivity.contentResolver.takePersistableUriPermission(uri, flags) } catch (_: Exception) {}
        invoke.resolve(
            JSObject()
                .put("uri", uri.toString())
                .put("name", DocumentFile.fromTreeUri(hostActivity, uri)?.name ?: "Selected folder")
        )
    }

    @ActivityCallback
    private fun safJsonResult(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().put("cancelled", true))
            return
        }
        worker.execute {
            try {
                val file = tempFile("session-import", ".json")
                hostActivity.contentResolver.openInputStream(uri).use { input ->
                    requireNotNull(input) { "Cannot open selected JSON" }
                    FileOutputStream(file).use { output -> input.copyTo(output) }
                }
                invoke.resolve(
                    JSObject()
                        .put("path", file.absolutePath)
                        .put("name", DocumentFile.fromSingleUri(hostActivity, uri)?.name ?: file.name)
                        .put("cancelled", false)
                )
            } catch (error: Exception) {
                invoke.reject(error.message ?: "JSON import failed")
            }
        }
    }

    @Command
    fun saf_release_folder(invoke: Invoke) {
        val args = invoke.parseArgs(TreeArgs::class.java)
        args.treeUri?.let {
            try {
                hostActivity.contentResolver.releasePersistableUriPermission(
                    Uri.parse(it),
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            } catch (_: Exception) {}
        }
        invoke.resolve(JSObject().put("released", true))
    }

    @Command
    fun saf_validate(invoke: Invoke) {
        try {
            val uri = Uri.parse(required(invoke.parseArgs(TreeArgs::class.java).treeUri, "treeUri"))
            val permission = hostActivity.contentResolver.persistedUriPermissions.firstOrNull {
                it.uri == uri && it.isReadPermission && it.isWritePermission
            }
            val folder = DocumentFile.fromTreeUri(hostActivity, uri)
            val valid = permission != null &&
                folder != null &&
                folder.exists() &&
                folder.isDirectory &&
                folder.canRead() &&
                folder.canWrite()
            invoke.resolve(
                JSObject()
                    .put("valid", valid)
                    .put("name", folder?.name ?: "")
            )
        } catch (_: Exception) {
            invoke.resolve(JSObject().put("valid", false).put("name", ""))
        }
    }

    @Command
    fun saf_list(invoke: Invoke) {
        try {
            val tree = tree(invoke.parseArgs(TreeArgs::class.java).treeUri)
            val items = JSArray()
            tree.listFiles().forEach { file ->
                items.put(
                    JSObject()
                        .put("name", file.name ?: "")
                        .put("directory", file.isDirectory)
                        .put("size", file.length())
                        .put("uri", file.uri.toString())
                )
            }
            invoke.resolve(JSObject().put("items", items))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "SAF list failed")
        }
    }

    @Command
    fun saf_read_to_temp(invoke: Invoke) {
        worker.execute {
            try {
                val args = invoke.parseArgs(SafPathArgs::class.java)
                val source = resolve(tree(args.treeUri), required(args.relativePath, "relativePath"), false)
                    ?: error("SAF file does not exist")
                val file = tempFile("saf-import", extension(source.name ?: ""))
                hostActivity.contentResolver.openInputStream(source.uri).use { input ->
                    requireNotNull(input) { "Cannot open SAF input stream" }
                    FileOutputStream(file).use { output -> input.copyTo(output) }
                }
                invoke.resolve(JSObject().put("path", file.absolutePath).put("size", file.length()))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "SAF read failed")
            }
        }
    }

    @Command
    fun saf_copy_tree_to_temp(invoke: Invoke) {
        worker.execute {
            try {
                val source = tree(invoke.parseArgs(TreeArgs::class.java).treeUri)
                val target = File(hostActivity.cacheDir, "palmannotate/import-${System.nanoTime()}")
                require(target.mkdirs()) { "Cannot create SAF import staging directory" }
                copyDocumentTree(source, target)
                invoke.resolve(JSObject().put("path", target.absolutePath))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "SAF folder import failed")
            }
        }
    }

    @Command
    fun saf_copy_from_path(invoke: Invoke) = copyToSaf(invoke)

    @Command
    fun saf_write(invoke: Invoke) = copyToSaf(invoke)

    private fun copyToSaf(invoke: Invoke) {
        worker.execute {
            try {
                val args = invoke.parseArgs(SafCopyArgs::class.java)
                val source = File(required(args.sourcePath, "sourcePath"))
                require(source.isFile) { "Source file does not exist" }
                val relative = required(args.relativePath, "relativePath")
                val parent = resolveParent(tree(args.treeUri), relative)
                val name = relative.substringAfterLast('/')
                parent.findFile(name)?.delete()
                val target = parent.createFile(args.mimeType ?: mimeFor(name), name)
                    ?: error("Cannot create SAF file")
                hostActivity.contentResolver.openOutputStream(target.uri).use { output ->
                    requireNotNull(output) { "Cannot open SAF output stream" }
                    FileInputStream(source).use { input -> input.copyTo(output) }
                }
                invoke.resolve(JSObject().put("ok", true).put("uri", target.uri.toString()))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "SAF write failed")
            }
        }
    }

    @Command
    fun saf_exists(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SafPathArgs::class.java)
            val exists = resolve(tree(args.treeUri), required(args.relativePath, "relativePath"), false) != null
            invoke.resolve(JSObject().put("exists", exists))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "SAF exists failed")
        }
    }

    @Command
    fun saf_delete(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SafPathArgs::class.java)
            val node = resolve(tree(args.treeUri), required(args.relativePath, "relativePath"), false)
            invoke.resolve(JSObject().put("removed", node?.delete() == true))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "SAF delete failed")
        }
    }

    private fun hasCameraPermission() =
        ContextCompat.checkSelfPermission(hostActivity, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED

    private fun emitCameraPreview(frame: ImageProxy) {
        try {
            val now = System.currentTimeMillis()
            if (now - lastCameraPreviewAt < 250L) return
            lastCameraPreviewAt = now
            val bitmap = frame.toBitmap()
            val preview = if (bitmap.width > 640) {
                Bitmap.createScaledBitmap(
                    bitmap,
                    640,
                    (bitmap.height * (640f / bitmap.width)).toInt().coerceAtLeast(1),
                    true
                )
            } else {
                bitmap
            }
            try {
                ByteArrayOutputStream().use { output ->
                    preview.compress(Bitmap.CompressFormat.JPEG, 55, output)
                    trigger(
                        "camera-preview",
                        JSObject()
                            .put("jpegBase64", Base64.encodeToString(output.toByteArray(), Base64.NO_WRAP))
                            .put("width", preview.width)
                            .put("height", preview.height)
                    )
                }
            } finally {
                if (preview !== bitmap) preview.recycle()
                bitmap.recycle()
            }
        } catch (_: Exception) {
            // Preview is optional; full-resolution capture remains available.
        } finally {
            frame.close()
        }
    }

    private fun usbManager() = hostActivity.getSystemService(Context.USB_SERVICE) as? UsbManager

    private fun orbbecDevices(): List<UsbDevice> =
        usbManager()?.deviceList?.values?.filter { it.vendorId == ORBBEC_VENDOR_ID } ?: emptyList()

    private fun openOrbbecLocked(): JSObject {
        if (obPipeline != null) {
            return JSObject()
                .put("opened", true)
                .put("alreadyOpen", true)
                .put("depthEnabled", obDepth)
                .put("uid", selectedUid ?: "")
        }
        val manager = usbManager() ?: error("USB service unavailable")
        val usb = orbbecDevices().firstOrNull() ?: error("No Orbbec device found")
        if (!manager.hasPermission(usb)) error("USB permission is required")

        OBContext.setLoggerSeverity(LogSeverity.INFO)
        val context = OBContext(hostActivity.applicationContext, deviceChangedCallback)
        val devices = queryOrbbecDevices(context)
        var openedDevice: Device? = null
        var openedPipeline: Pipeline? = null
        var config: Config? = null
        var colorProfile: VideoStreamProfile? = null
        var depthProfile: VideoStreamProfile? = null
        try {
            if (devices.deviceCount <= 0) error("Orbbec SDK cannot see the USB device")
            var index = 0
            for (candidate in 0 until devices.deviceCount) {
                if (devices.getVid(candidate) == ORBBEC_VENDOR_ID) {
                    index = candidate
                    break
                }
            }
            val uid = devices.getUid(index) ?: ""
            openedDevice = devices.getDevice(index) ?: error("Failed to open Orbbec device")
            val name = openedDevice.getInfo()?.getName() ?: "Orbbec camera"
            openedPipeline = Pipeline(openedDevice)
            config = Config()

            colorProfile = chooseColorProfile(openedPipeline)
            if (colorProfile != null) config.enableStream(colorProfile)
            else config.enableStream(SensorType.COLOR)

            var depthEnabled = false
            try {
                depthProfile = chooseDepthProfile(openedPipeline)
                if (depthProfile != null) config.enableStream(depthProfile)
                else config.enableStream(SensorType.DEPTH)
                try {
                    config.setAlignMode(AlignMode.ALIGN_D2C_SW_MODE)
                } catch (error: Exception) {
                    Log.w(TAG, "D2C alignment unavailable", error)
                }
                try { config.setDepthScaleRequire(true) } catch (_: Exception) {}
                try {
                    config.setFrameAggregateOutputMode(
                        FrameAggregateOutputMode.OB_FRAME_AGGREGATE_OUTPUT_ALL_TYPE_FRAME_REQUIRE
                    )
                } catch (_: Exception) {}
                depthEnabled = true
            } catch (error: Exception) {
                Log.w(TAG, "Depth stream unavailable; using RGB only", error)
                safeClose(depthProfile)
                depthProfile = null
            }

            openedPipeline.start(config)
            obContext = context
            obDevice = openedDevice
            obPipeline = openedPipeline
            obDepth = depthEnabled
            selectedUid = uid
            openedDevice = null
            openedPipeline = null

            return JSObject()
                .put("opened", true)
                .put("alreadyOpen", false)
                .put("uid", uid)
                .put("name", name)
                .put("width", colorProfile?.width ?: 0)
                .put("height", colorProfile?.height ?: 0)
                .put("fps", colorProfile?.fps ?: 0)
                .put("sourceFormat", colorProfile?.format?.name ?: "default")
                .put("depthEnabled", depthEnabled)
                .put("depthWidth", depthProfile?.width ?: 0)
                .put("depthHeight", depthProfile?.height ?: 0)
                .put("depthFps", depthProfile?.fps ?: 0)
                .put("depthFormat", depthProfile?.format?.name ?: "none")
        } catch (error: Exception) {
            safeStopAndClose(openedPipeline)
            safeClose(openedDevice)
            safeClose(context)
            throw error
        } finally {
            safeClose(colorProfile)
            safeClose(depthProfile)
            safeClose(config)
            safeClose(devices)
        }
    }

    private fun queryOrbbecDevices(context: OBContext): DeviceList {
        var latest: DeviceList? = null
        repeat(DEVICE_QUERY_RETRIES) {
            safeClose(latest)
            latest = context.queryDevices()
            if (latest!!.deviceCount > 0) return latest!!
            try {
                Thread.sleep(DEVICE_QUERY_DELAY_MS)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                return latest!!
            }
        }
        return latest ?: context.queryDevices()
    }

    private fun chooseColorProfile(pipeline: Pipeline): VideoStreamProfile? {
        val list: StreamProfileList = pipeline.getStreamProfileList(SensorType.COLOR) ?: return null
        val profiles = ArrayList<VideoStreamProfile>()
        try {
            for (index in 0 until list.count) {
                val profile: VideoStreamProfile =
                    list.getProfile(index).`as`(StreamType.VIDEO)
                if (profile.width >= 640 && profile.height >= 360 &&
                    isCapturableColorFormat(profile.format)
                ) {
                    profiles.add(profile)
                } else {
                    safeClose(profile)
                }
            }
        } finally {
            safeClose(list)
        }
        if (profiles.isEmpty()) return null
        profiles.sortWith(
            compareBy<VideoStreamProfile> { colorFormatPriority(it.format) }
                .thenBy { kotlin.math.abs(it.width - 1280) }
                .thenByDescending { it.fps }
                .thenByDescending { it.width * it.height }
        )
        return profiles.first().also { selected ->
            profiles.drop(1).forEach(::safeClose)
            Log.i(TAG, "Selected RGB ${selected.width}x${selected.height}@${selected.fps} ${selected.format}")
        }
    }

    private fun chooseDepthProfile(pipeline: Pipeline): VideoStreamProfile? {
        val list: StreamProfileList = pipeline.getStreamProfileList(SensorType.DEPTH) ?: return null
        val profiles = ArrayList<VideoStreamProfile>()
        try {
            for (index in 0 until list.count) {
                val profile: VideoStreamProfile =
                    list.getProfile(index).`as`(StreamType.VIDEO)
                if (profile.width >= 320 && profile.height >= 240 &&
                    isCapturableDepthFormat(profile.format)
                ) {
                    profiles.add(profile)
                } else {
                    safeClose(profile)
                }
            }
        } finally {
            safeClose(list)
        }
        if (profiles.isEmpty()) return null
        profiles.sortWith(
            compareBy<VideoStreamProfile> { depthFormatPriority(it.format) }
                .thenBy { kotlin.math.abs(it.width - 1280) }
                .thenByDescending { it.fps }
                .thenByDescending { it.width * it.height }
        )
        return profiles.first().also { selected ->
            profiles.drop(1).forEach(::safeClose)
            Log.i(TAG, "Selected depth ${selected.width}x${selected.height}@${selected.fps} ${selected.format}")
        }
    }

    private fun startOrbbecPreview() {
        synchronized(orbbecLock) {
            if (orbbecPreviewRunning) return
            orbbecPreviewRunning = true
            streamPump = Thread(::runOrbbecPreview, "PalmAnnotate-OrbbecPump").apply {
                isDaemon = true
                start()
            }
        }
    }

    private fun stopOrbbecPreview() {
        orbbecPreviewRunning = false
        pendingCapture.getAndSet(null)?.reject(IllegalStateException("Orbbec preview stopped"))
        streamPump?.interrupt()
    }

    private fun joinOrbbecPreview() {
        orbbecPreviewRunning = false
        val thread = synchronized(orbbecLock) { streamPump }
        if (thread != null && thread.isAlive && thread !== Thread.currentThread()) {
            thread.interrupt()
            try {
                thread.join(FRAME_TIMEOUT_MS + 1_000L)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
            }
        }
        synchronized(orbbecLock) {
            if (streamPump === thread) streamPump = null
        }
    }

    private fun runOrbbecPreview() {
        var lastPreviewAt = 0L
        while (orbbecPreviewRunning) {
            val pipeline = synchronized(orbbecLock) { obPipeline } ?: break
            var frames: FrameSet? = null
            var color: ColorFrame? = null
            var depth: DepthFrame? = null
            try {
                frames = pipeline.waitForFrameSet(FRAME_TIMEOUT_MS) ?: continue
                color = frames.colorFrame
                depth = frames.depthFrame

                pendingCapture.getAndSet(null)?.let { waiter ->
                    try {
                        val rgb = color ?: error("Orbbec RGB frame is missing")
                        if (obDepth && depth == null) error("Orbbec depth frame is missing")
                        waiter.resolve(encodeCapturedFrame(rgb, depth))
                    } catch (error: Exception) {
                        waiter.reject(error)
                    }
                }

                val now = System.currentTimeMillis()
                if (now - lastPreviewAt >= 250L) {
                    lastPreviewAt = now
                    val event = JSObject()
                    color?.let {
                        event.put(
                            "rgbJpegBase64",
                            Base64.encodeToString(previewJpeg(encodeColor(it)), Base64.NO_WRAP)
                        )
                    }
                    depth?.let {
                        event.put(
                            "depthJpegBase64",
                            Base64.encodeToString(depthPreviewJpeg(it), Base64.NO_WRAP)
                        )
                    }
                    if (color != null || depth != null) trigger("orbbec-preview", event)
                }
            } catch (_: InterruptedException) {
                break
            } catch (error: Exception) {
                pendingCapture.getAndSet(null)?.reject(error)
                if (orbbecPreviewRunning) Log.w(TAG, "Orbbec frame pump failed", error)
            } finally {
                safeClose(depth)
                safeClose(color)
                safeClose(frames)
            }
        }
    }

    private fun captureViaPump(): CapturedRgbd {
        if (!orbbecPreviewRunning) return captureOrbbecDirect()
        val waiter = CaptureWaiter()
        if (!pendingCapture.compareAndSet(null, waiter)) {
            error("Another Orbbec capture is already pending")
        }
        return try {
            waiter.await()
        } finally {
            pendingCapture.compareAndSet(waiter, null)
        }
    }

    private fun captureOrbbecDirect(): CapturedRgbd {
        val pipeline = synchronized(orbbecLock) {
            obPipeline ?: error("Orbbec camera is not open")
        }
        var lastError: Exception? = null
        repeat(3) {
            var frames: FrameSet? = null
            var color: ColorFrame? = null
            var depth: DepthFrame? = null
            try {
                frames = pipeline.waitForFrameSet(FRAME_TIMEOUT_MS) ?: return@repeat
                color = frames.colorFrame ?: return@repeat
                depth = frames.depthFrame
                if (obDepth && depth == null) return@repeat
                return encodeCapturedFrame(color, depth)
            } catch (error: Exception) {
                lastError = error
            } finally {
                safeClose(depth)
                safeClose(color)
                safeClose(frames)
            }
        }
        throw IllegalStateException(lastError?.message ?: "No Orbbec RGB-D frame received")
    }

    private fun encodeCapturedFrame(color: ColorFrame, depth: DepthFrame?): CapturedRgbd =
        CapturedRgbd(
            CapturedJpeg(encodeColor(color), color.width, color.height, color.format.name),
            depth?.let {
                CapturedDepth(copyFrame(it), it.width, it.height, it.format.name, it.valueScale)
            }
        )

    private fun writeCapturedFrame(frame: CapturedRgbd): JSObject {
        val colorFile = tempFile("orbbec-rgb", ".jpg")
        FileOutputStream(colorFile).use { it.write(frame.color.bytes) }
        val result = JSObject()
            .put("path", colorFile.absolutePath)
            .put("width", frame.color.width)
            .put("height", frame.color.height)
            .put("format", "jpeg")
            .put("source", "orbbec")
            .put("sourceFormat", frame.color.sourceFormat)
            .put("hasDepth", frame.depth != null)
        frame.depth?.let { depth ->
            val depthFile = tempFile("orbbec-depth", ".raw")
            FileOutputStream(depthFile).use { it.write(depth.bytes) }
            val metadata = File(depthFile.absolutePath + ".json")
            metadata.writeText(
                """{"width":${depth.width},"height":${depth.height},"format":"${depth.sourceFormat}","valueScale":${depth.valueScale},"dataType":"uint16_le","alignedTo":"color"}"""
            )
            result
                .put("depthPath", depthFile.absolutePath)
                .put("depthMetadataPath", metadata.absolutePath)
                .put("depthWidth", depth.width)
                .put("depthHeight", depth.height)
                .put("depthFormat", depth.sourceFormat)
                .put("depthValueScale", depth.valueScale)
        }
        return result
    }

    private fun closeOrbbecLocked() {
        val pipeline = obPipeline
        val device = obDevice
        val context = obContext
        obPipeline = null
        obDevice = null
        obContext = null
        obDepth = false
        selectedUid = null
        pendingCapture.getAndSet(null)?.reject(IllegalStateException("Orbbec camera closed"))
        safeStopAndClose(pipeline)
        safeClose(device)
        safeClose(context)
    }

    private fun previewJpeg(jpeg: ByteArray): ByteArray {
        val bitmap = BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)
            ?: error("Cannot decode Orbbec RGB preview")
        val preview = if (bitmap.width > 640) {
            Bitmap.createScaledBitmap(
                bitmap,
                640,
                (bitmap.height * (640f / bitmap.width)).toInt().coerceAtLeast(1),
                true
            )
        } else {
            bitmap
        }
        return try {
            ByteArrayOutputStream().use { output ->
                preview.compress(Bitmap.CompressFormat.JPEG, 55, output)
                output.toByteArray()
            }
        } finally {
            if (preview !== bitmap) preview.recycle()
            bitmap.recycle()
        }
    }

    private fun depthPreviewJpeg(frame: DepthFrame): ByteArray {
        val raw = copyFrame(frame)
        val count = minOf(raw.size / 2, frame.width * frame.height)
        var minimum = 0xffff
        var maximum = 0
        for (index in 0 until count) {
            val value = (raw[index * 2].toInt() and 0xff) or
                ((raw[index * 2 + 1].toInt() and 0xff) shl 8)
            if (value > 0) {
                minimum = minOf(minimum, value)
                maximum = maxOf(maximum, value)
            }
        }
        if (minimum == 0xffff) minimum = 0
        val span = maxOf(1, maximum - minimum)
        val pixels = IntArray(frame.width * frame.height)
        for (index in 0 until count) {
            val value = (raw[index * 2].toInt() and 0xff) or
                ((raw[index * 2 + 1].toInt() and 0xff) shl 8)
            val gray = if (value == 0) 0 else ((value - minimum) * 255 / span).coerceIn(0, 255)
            pixels[index] = (0xff shl 24) or (gray shl 16) or (gray shl 8) or gray
        }
        val bitmap = Bitmap.createBitmap(
            pixels,
            frame.width,
            frame.height,
            Bitmap.Config.ARGB_8888
        )
        return try {
            ByteArrayOutputStream().use { output ->
                bitmap.compress(Bitmap.CompressFormat.JPEG, 50, output)
                previewJpeg(output.toByteArray())
            }
        } finally {
            bitmap.recycle()
        }
    }

    private fun copyFrame(frame: com.orbbec.obsensor.Frame): ByteArray {
        val raw = ByteArray(frame.dataSize)
        val copied = frame.getData(raw)
        if (copied < 0) error("Failed to copy Orbbec frame")
        return if (copied in 0 until raw.size) raw.copyOf(copied) else raw
    }

    private fun encodeColor(frame: ColorFrame): ByteArray {
        val data = copyFrame(frame)
        return when (frame.format) {
            Format.MJPG -> data
            Format.RGB, Format.BGR, Format.RGBA, Format.BGRA -> {
                val stride = if (frame.format == Format.RGBA || frame.format == Format.BGRA) 4 else 3
                val pixels = IntArray(frame.width * frame.height)
                var source = 0
                for (index in pixels.indices) {
                    val a = data[source].toInt() and 0xff
                    val b = data[source + 1].toInt() and 0xff
                    val c = data[source + 2].toInt() and 0xff
                    val rgb = if (frame.format == Format.RGB || frame.format == Format.RGBA) {
                        Triple(a, b, c)
                    } else {
                        Triple(c, b, a)
                    }
                    pixels[index] = (0xff shl 24) or (rgb.first shl 16) or (rgb.second shl 8) or rgb.third
                    source += stride
                }
                val bitmap = Bitmap.createBitmap(pixels, frame.width, frame.height, Bitmap.Config.ARGB_8888)
                try {
                    ByteArrayOutputStream().use { output ->
                        bitmap.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, output)
                        output.toByteArray()
                    }
                } finally {
                    bitmap.recycle()
                }
            }
            Format.YUYV, Format.YUY2 -> yuvJpeg(data, ImageFormat.YUY2, frame.width, frame.height)
            Format.NV21 -> yuvJpeg(data, ImageFormat.NV21, frame.width, frame.height)
            Format.NV12 -> {
                val ySize = frame.width * frame.height
                val nv21 = data.copyOf()
                var index = ySize
                while (index + 1 < nv21.size) {
                    val u = nv21[index]
                    nv21[index] = nv21[index + 1]
                    nv21[index + 1] = u
                    index += 2
                }
                yuvJpeg(nv21, ImageFormat.NV21, frame.width, frame.height)
            }
            else -> error("Unsupported Orbbec color format: ${frame.format}")
        }
    }

    private fun yuvJpeg(data: ByteArray, format: Int, width: Int, height: Int): ByteArray {
        val output = ByteArrayOutputStream()
        require(YuvImage(data, format, width, height, null).compressToJpeg(
            Rect(0, 0, width, height),
            JPEG_QUALITY,
            output
        )) { "Failed to encode Orbbec YUV frame" }
        return output.toByteArray()
    }

    private fun isCapturableColorFormat(format: Format): Boolean = when (format) {
        Format.MJPG, Format.RGB, Format.BGR, Format.RGBA, Format.BGRA,
        Format.YUYV, Format.YUY2, Format.NV21, Format.NV12 -> true
        else -> false
    }

    private fun colorFormatPriority(format: Format): Int = when (format) {
        Format.MJPG -> 0
        Format.RGB, Format.BGR -> 1
        Format.RGBA, Format.BGRA -> 2
        Format.NV12, Format.NV21 -> 3
        Format.YUYV, Format.YUY2 -> 4
        else -> 99
    }

    private fun isCapturableDepthFormat(format: Format): Boolean = when (format) {
        Format.Y16, Format.Y12, Format.Y11, Format.Y10 -> true
        else -> false
    }

    private fun depthFormatPriority(format: Format): Int = when (format) {
        Format.Y16 -> 0
        Format.Y12 -> 1
        Format.Y11 -> 2
        Format.Y10 -> 3
        else -> 99
    }

    private fun safeStopAndClose(pipeline: Pipeline?) {
        if (pipeline == null) return
        try { pipeline.stop() } catch (_: Exception) {}
        safeClose(pipeline)
    }

    private fun safeClose(value: AutoCloseable?) {
        try { value?.close() } catch (_: Exception) {}
    }

    private fun tree(uri: String?): DocumentFile =
        DocumentFile.fromTreeUri(hostActivity, Uri.parse(required(uri, "treeUri")))
            ?: error("SAF folder is inaccessible")

    private fun resolve(root: DocumentFile, relative: String, createDirectories: Boolean): DocumentFile? {
        var node = root
        val segments = relative.split('/').filter { it.isNotBlank() }
        segments.forEachIndexed { index, segment ->
            val existing = node.findFile(segment)
            if (existing != null) {
                node = existing
            } else if (createDirectories || index < segments.lastIndex) {
                node = node.createDirectory(segment) ?: return null
            } else {
                return null
            }
        }
        return node
    }

    private fun resolveParent(root: DocumentFile, relative: String): DocumentFile {
        val parent = relative.substringBeforeLast('/', "")
        return if (parent.isBlank()) root
        else resolve(root, parent, true) ?: error("Cannot create SAF parent folder")
    }

    private fun copyDocumentTree(source: DocumentFile, target: File) {
        source.listFiles().forEach { child ->
            val name = child.name?.takeIf {
                it.isNotBlank() && it != "." && it != ".." &&
                    !it.contains('/') && !it.contains('\\')
            } ?: return@forEach
            val destination = File(target, name)
            if (child.isDirectory) {
                require(destination.mkdirs() || destination.isDirectory) {
                    "Cannot create import directory $name"
                }
                copyDocumentTree(child, destination)
            } else if (child.isFile) {
                hostActivity.contentResolver.openInputStream(child.uri).use { input ->
                    requireNotNull(input) { "Cannot read SAF file $name" }
                    FileOutputStream(destination).use { output -> input.copyTo(output) }
                }
            }
        }
    }

    private fun required(value: String?, name: String): String =
        value?.takeIf { it.isNotBlank() } ?: error("$name is required")

    private fun tempFile(prefix: String, suffix: String): File {
        val directory = File(hostActivity.cacheDir, "palmannotate").apply { mkdirs() }
        return File.createTempFile("$prefix-", suffix, directory)
    }

    private fun extension(name: String): String {
        val index = name.lastIndexOf('.')
        return if (index >= 0) name.substring(index) else ".tmp"
    }

    private fun mimeFor(name: String): String = when {
        name.endsWith(".json", true) -> "application/json"
        name.endsWith(".jpg", true) || name.endsWith(".jpeg", true) -> "image/jpeg"
        name.endsWith(".png", true) -> "image/png"
        name.endsWith(".txt", true) -> "text/plain"
        name.endsWith(".raw", true) -> "application/octet-stream"
        else -> "application/octet-stream"
    }

    override fun onDestroy(activity: AppCompatActivity) {
        cameraProvider?.unbindAll()
        cameraAnalysis = null
        stopOrbbecPreview()
        joinOrbbecPreview()
        synchronized(orbbecLock) { closeOrbbecLocked() }
        worker.shutdownNow()
        super.onDestroy(activity)
    }
}
