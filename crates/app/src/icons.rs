//! The curated Lucide icons a Project can use, and the asset source that
//! embeds them next to gpui-kit's default component icons.

use std::borrow::Cow;

use gpui_kit::assets::{Assets, IconName};
use gpui_kit::{AssetSource, Result, SharedString};

/// Lists each icon once and expands to both the embedded asset source and the
/// picker's list, so the two can't drift apart.
macro_rules! project_icons {
    ($($icon:ident),* $(,)?) => {
        gpui_kit::assets::icon_assets!(ProjectIconAssets, [$($icon),*]);

        /// Every icon the picker offers, in picker order.
        pub(crate) const PROJECT_ICONS: &[IconName] = &[$(IconName::$icon),*];
    };
}

project_icons![
    // Work and planning
    Folder,
    FolderOpen,
    FolderKanban,
    Inbox,
    Briefcase,
    Building2,
    Rocket,
    Target,
    Flag,
    Star,
    Heart,
    Lightbulb,
    Sparkles,
    Zap,
    // Code
    Code,
    CodeXml,
    Terminal,
    SquareTerminal,
    Braces,
    GitBranch,
    GitPullRequest,
    Github,
    Bug,
    Database,
    Server,
    Cloud,
    Cpu,
    Bot,
    Binary,
    Package,
    Blocks,
    Puzzle,
    Webhook,
    Globe,
    Link,
    Wifi,
    // Devices
    Laptop,
    Monitor,
    Smartphone,
    Tablet,
    Keyboard,
    Mouse,
    Headphones,
    Watch,
    Printer,
    HardDrive,
    Camera,
    Gamepad2,
    Tv,
    // Home and family
    House,
    Sofa,
    Bed,
    Lamp,
    Hammer,
    Wrench,
    Drill,
    PaintRoller,
    Paintbrush,
    Palette,
    Shovel,
    Trees,
    Flower2,
    Sprout,
    Leaf,
    Dog,
    Cat,
    Baby,
    Users,
    User,
    UserRound,
    // Money
    Wallet,
    CreditCard,
    PiggyBank,
    Banknote,
    Receipt,
    Landmark,
    ChartLine,
    ChartPie,
    TrendingUp,
    Calculator,
    Coins,
    ShoppingCart,
    ShoppingBag,
    Gift,
    Tag,
    Store,
    // Health
    HeartPulse,
    Activity,
    Dumbbell,
    Bike,
    Footprints,
    Apple,
    Salad,
    Pill,
    Stethoscope,
    Brain,
    Moon,
    Sun,
    // Travel
    Plane,
    Car,
    TrainFront,
    Bus,
    Ship,
    Map,
    MapPin,
    Compass,
    Tent,
    Mountain,
    Luggage,
    Earth,
    // Writing and learning
    PenLine,
    Pencil,
    NotebookPen,
    Notebook,
    BookOpen,
    Book,
    Library,
    FileText,
    Newspaper,
    Scroll,
    Quote,
    Feather,
    Type,
    Languages,
    GraduationCap,
    School,
    Backpack,
    Microscope,
    FlaskConical,
    Atom,
    Telescope,
    // Media
    Music,
    Film,
    Clapperboard,
    Image,
    Mic,
    AudioLines,
    Radio,
    Video,
    Guitar,
    // Time and messages
    Calendar,
    Clock,
    AlarmClock,
    ListTodo,
    ListChecks,
    ClipboardList,
    Kanban,
    Mail,
    MessageCircle,
    Megaphone,
    Phone,
    Send,
    // Food
    Coffee,
    Utensils,
    ChefHat,
    Pizza,
    Wine,
    Beer,
    Cake,
    // Other
    Trophy,
    Medal,
    Award,
    Crown,
    Shield,
    Lock,
    Key,
    Settings,
    Cog,
    Box,
    Archive,
    Layers,
    LayoutDashboard,
];

gpui_kit::assets::icon_assets!(NavIconAssets, [LayoutTemplate]);

pub(crate) const TEMPLATES_ICON: IconName = IconName::LayoutTemplate;

/// What a Project shows when it has no icon, or one outside the curated set.
pub(crate) const DEFAULT_ICON: IconName = IconName::Folder;

/// The app's assets: the curated Project icons layered over gpui-kit's
/// default icons, which the components themselves use.
#[derive(Clone, Copy, Debug, Default)]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ProjectIconAssets.load(path)? {
            return Ok(Some(bytes));
        }
        match NavIconAssets.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = ProjectIconAssets.list(path)?;
        for p in NavIconAssets
            .list(path)?
            .into_iter()
            .chain(Assets.list(path)?)
        {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        Ok(paths)
    }
}

/// The kebab-case Lucide name stored in `project.md`, e.g. `folder-open`.
pub(crate) fn icon_name(icon: IconName) -> SharedString {
    let path = icon.path();
    path.strip_prefix("icons/")
        .and_then(|p| p.strip_suffix(".svg"))
        .map(|name| SharedString::from(name.to_string()))
        .unwrap_or(path)
}

/// The curated icon stored under `name`. Names outside the curated set (an
/// agent or hand edit) fall back to the folder, since only these are embedded.
pub(crate) fn project_icon(name: Option<&str>) -> IconName {
    name.and_then(|name| {
        PROJECT_ICONS
            .iter()
            .copied()
            .find(|icon| icon_name(*icon) == name)
    })
    .unwrap_or(DEFAULT_ICON)
}
