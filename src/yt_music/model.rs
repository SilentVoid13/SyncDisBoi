use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct YtMusicOAuthDeviceRes {
    pub verification_url: String,
    pub user_code: String,
    pub device_code: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicResponse {
    pub contents: ResponseContent,
}

impl YtMusicResponse {
    pub fn merge(&mut self, other: &mut YtMusicContinuationResponse) -> Option<()> {
        if let Some(gr) = self.get_grid_renderer() {
            gr.items.append(
                &mut other
                    .continuation_contents
                    .grid_continuation
                    .as_mut()?
                    .items,
            );
            Some(())
        } else if let Some(mpsr) = self.get_music_playlist_shelf_renderer() {
            mpsr.contents.as_mut()?.append(
                other
                    .continuation_contents
                    .music_playlist_shelf_continuation
                    .as_mut()?
                    .contents
                    .as_mut()?,
            );
            Some(())
        } else {
            None
        }
    }

    pub fn get_mrlirs(&mut self) -> Option<Vec<&MusicResponsiveListItemRenderer>> {
        let section_renderer_content = self.get_section_renderer_content()?;
        if let Some(mpsr) = &mut section_renderer_content.music_playlist_shelf_renderer {
            Some(
                mpsr.contents
                    .as_ref()?
                    .iter()
                    .map(|item| &item.music_responsive_list_item_renderer)
                    .collect(),
            )
        } else if let Some(msr) = &mut section_renderer_content.music_shelf_renderer {
            Some(
                msr.contents
                    .as_ref()?
                    .iter()
                    .map(|item| &item.music_responsive_list_item_renderer)
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn get_mtrirs(&mut self) -> Option<Vec<&MusicTwoRowItemRenderer>> {
        Some(
            self.get_grid_renderer()?
                .items
                .iter()
                .map(|item2| &item2.music_two_row_item_renderer)
                .collect(),
        )
    }

    pub fn get_section_renderer_content(&mut self) -> Option<&mut SectionRendererContent> {
        if let Some(sr) = self.contents.single_column_browse_results_renderer.as_mut() {
            sr.tabs[0]
                .tab_renderer
                .content
                .section_list_renderer
                .contents
                .as_mut()?
                .iter_mut()
                .find(|item| {
                    item.music_playlist_shelf_renderer.is_some() || item.grid_renderer.is_some()
                })
        } else if let Some(tr) = self.contents.tabbed_search_results_renderer.as_mut() {
            tr.tabs[0]
                .tab_renderer
                .content
                .section_list_renderer
                .contents
                .as_mut()?
                .iter_mut()
                .find(|item| {
                    item.music_playlist_shelf_renderer.is_some()
                        || item.grid_renderer.is_some()
                        || item.music_shelf_renderer.is_some()
                })
        } else if let Some(tr) = self.contents.two_column_browse_results_renderer.as_mut() {
            tr.secondary_contents
                .section_list_renderer
                .contents
                .as_mut()?
                .iter_mut()
                .find(|item| {
                    item.music_playlist_shelf_renderer.is_some() || item.grid_renderer.is_some()
                })
        } else {
            None
        }
    }

    pub fn get_grid_renderer(&mut self) -> Option<&mut GridRenderer> {
        self.get_section_renderer_content()?.grid_renderer.as_mut()
    }

    pub fn get_music_playlist_shelf_renderer(&mut self) -> Option<&mut MusicPlaylistShelfRenderer> {
        self.get_section_renderer_content()?
            .music_playlist_shelf_renderer
            .as_mut()
    }

    pub fn get_continuation(&mut self) -> Option<String> {
        if let Some(gr) = self.get_grid_renderer() {
            Some(gr.continuations.as_ref()?[0].get_continuation())
        } else if let Some(mpsr) = self.get_music_playlist_shelf_renderer() {
            Some(mpsr.continuations.as_ref()?[0].get_continuation())
        } else {
            None
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentsVec<T> {
    pub contents: Option<Vec<T>>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentsSingle<T> {
    pub contents: [T; 1],
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Content<T> {
    pub content: T,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResponseContent {
    pub two_column_browse_results_renderer: Option<TwoColumnBrowseResultsRenderer>,
    pub single_column_browse_results_renderer: Option<SingleColumnBrowseResultsRenderer>,
    pub tabbed_search_results_renderer: Option<TabbedSearchResultsRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnBrowseResultsRenderer {
    pub secondary_contents: TabRendererContent,
    pub tabs: [Tab; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SingleColumnBrowseResultsRenderer {
    pub tabs: [Tab; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: Content<TabRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererContent {
    pub section_list_renderer: ContentsVec<SectionRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SectionRendererContent {
    pub music_playlist_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
    //pub item_section_renderer: Option<ContentsSingle<ItemSectionRendererContent>>,
    pub grid_renderer: Option<GridRenderer>,
    pub music_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRenderer {
    pub contents: Option<Vec<MusicPlaylistShelfRendererContent>>,
    pub continuations: Option<[Continuation; 1]>,
}
impl MusicPlaylistShelfRenderer {
    pub fn get_continuation(&self) -> Option<String> {
        Some(self.continuations.as_ref()?[0].get_continuation())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRendererContent {
    pub music_responsive_list_item_renderer: MusicResponsiveListItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemRenderer {
    pub menu: Option<Menu>,
    pub overlay: Option<Overlay>,
    pub flex_columns: Vec<FlexColumn>,
    pub fixed_columns: Option<Vec<FixedColumn>>,
    pub playlist_item_data: Option<PlaylistItemData>,
}
impl MusicResponsiveListItemRenderer {
    pub fn get_set_id(&self) -> Option<String> {
        self.playlist_item_data
            .as_ref()?
            .playlist_set_video_id
            .clone()
    }

    pub fn get_id(&self) -> Option<String> {
        Some(self.playlist_item_data.as_ref()?.video_id.clone())
    }

    pub fn get_col_run_text(&self, idx: usize, run_i: usize, flex: bool) -> Option<String> {
        Some(self.get_col_runs(idx, flex)?.get(run_i)?.get_text())
    }

    pub fn get_col_run_id(&self, idx: usize, run_i: usize, flex: bool) -> Option<String> {
        self.get_col_runs(idx, flex)?.get(run_i)?.get_id()
    }

    pub fn get_col_runs(&self, idx: usize, flex: bool) -> Option<&Vec<Run>> {
        let mrlifcr = if flex {
            &self
                .flex_columns
                .get(idx)?
                .music_responsive_list_item_flex_column_renderer
        } else {
            &self
                .fixed_columns
                .as_ref()?
                .get(idx)?
                .music_responsive_list_item_fixed_column_renderer
        };
        mrlifcr.text.runs.as_ref()
    }

    #[allow(dead_code)]
    fn get_action(&self) -> Option<&Action> {
        Some(
            &self
                .menu
                .as_ref()?
                .menu_renderer
                .items
                .iter()
                .filter_map(|item| {
                    Some(&item.menu_service_item_renderer.as_ref()?.service_endpoint)
                })
                .find(|item| item.playlist_edit_endpoint.is_some())?
                .playlist_edit_endpoint
                .as_ref()?
                .actions[0],
        )
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRendererContent {
    pub grid_renderer: Option<GridRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GridRenderer {
    pub items: Vec<Item2>,
    pub continuations: Option<[Continuation; 1]>,
}
impl GridRenderer {
    pub fn get_continuation(&self) -> Option<String> {
        Some(self.continuations.as_ref()?[0].get_continuation())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item2 {
    pub music_two_row_item_renderer: MusicTwoRowItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicTwoRowItemRenderer {
    pub menu: Option<Menu>,
    pub title: Text,
}
impl MusicTwoRowItemRenderer {
    pub fn get_id(&self) -> Option<String> {
        self.title.runs.as_ref()?.first()?.get_id()
    }

    pub fn get_name(&self) -> Option<String> {
        Some(self.title.runs.as_ref()?.first()?.get_text())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    pub next_continuation_data: NextContinuationData,
}
impl Continuation {
    pub fn get_continuation(&self) -> String {
        self.next_continuation_data.continuation.clone()
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NextContinuationData {
    pub continuation: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumn {
    pub music_responsive_list_item_flex_column_renderer: MusicResponsiveListItemFlexColumnRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FixedColumn {
    pub music_responsive_list_item_fixed_column_renderer: MusicResponsiveListItemFlexColumnRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemFlexColumnRenderer {
    pub text: Text,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub runs: Option<Vec<Run>>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub text: String,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}
impl Run {
    pub fn get_text(&self) -> String {
        self.text.clone()
    }

    pub fn get_id(&self) -> Option<String> {
        Some(
            self.navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?
                .browse_id
                .clone(),
        )
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub browse_endpoint: Option<BrowseEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub menu_renderer: MenuRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuRenderer {
    pub items: Vec<Item>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub menu_service_item_renderer: Option<MenuServiceItemRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuServiceItemRenderer {
    pub service_endpoint: ServiceEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEndpoint {
    pub playlist_edit_endpoint: Option<PlaylistEditEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEditEndpoint {
    pub actions: [Action; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub action: String,
    pub removed_video_id: String,
    pub set_video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    pub music_item_thumbnail_overlay_renderer: Content<MusicItemThumbnailOverlayRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicItemThumbnailOverlayRendererContent {
    pub music_play_button_renderer: MusicPlayButtonRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlayButtonRenderer {
    pub play_navigation_endpoint: Option<PlayNavigationEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayNavigationEndpoint {
    pub watch_endpoint: WatchEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatchEndpoint {
    pub playlist_id: Option<String>,
    pub video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub playlist_set_video_id: Option<String>,
    pub video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicContinuationResponse {
    pub continuation_contents: ContinuationContents,
}
impl YtMusicContinuationResponse {
    pub fn get_continuation(&self) -> Option<String> {
        if let Some(g) = &self.continuation_contents.grid_continuation {
            g.get_continuation()
        } else if let Some(m) = &self.continuation_contents.music_playlist_shelf_continuation {
            m.get_continuation()
        } else {
            None
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationContents {
    pub grid_continuation: Option<GridRenderer>,
    pub music_playlist_shelf_continuation: Option<MusicPlaylistShelfRenderer>,
}

// Responses

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommandContent<T> {
    pub command_executor_command: CommandExecutorCommand<T>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutorCommand<T> {
    pub commands: Vec<CommandsContent<T>>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum CommandsContent<T> {
    Cmd(T),
    Other(serde_json::Value),
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TabbedSearchResultsRenderer {
    pub tabs: Vec<Tab>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicPlaylistCreateResponse {
    pub playlist_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicPlaylistEditResponse {
    pub status: String,
}
impl YtMusicPlaylistEditResponse {
    pub fn success(&self) -> bool {
        self.status == "STATUS_SUCCEEDED"
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicPlaylistDeleteResponse {
    pub command: CommandContent<DeleteCommand>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCommand {
    pub handle_playlist_deletion_command: HandlePlaylistDeletionCommand,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HandlePlaylistDeletionCommand {
    pub playlist_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicAddLikeResponse {
    pub response_context: YtMusicResponseContext,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicResponseContext {
    pub visitor_data: String,
}
