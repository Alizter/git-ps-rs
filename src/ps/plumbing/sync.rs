use super::git;
use super::super::super::ps;
use super::super::super::ps::state_management;

#[derive(Debug)]
pub enum SyncError {
  RepositoryNotFound,
  CurrentBranchNameMissing,
  GetUpstreamBranchNameFailed,
  GetRemoteBranchNameFailed,
  CreateRrBranchFailed(ps::plumbing::branch::BranchError),
  RequestReviewBranchNameMissing,
  ForcePushFailed(ps::plumbing::git::ExtForcePushError),
  StorePatchStateFailed(state_management::StorePatchStateError)
}

pub fn sync(patch_index: usize) -> Result<(), SyncError> {
  let repo = git::create_cwd_repo().map_err(|_| SyncError::RepositoryNotFound)?;

  // get remote name of current branch
  let cur_branch_name = git::get_current_branch(&repo).ok_or(SyncError::CurrentBranchNameMissing)?;
  let branch_upstream_name = git::branch_upstream_name(&repo, cur_branch_name.as_str()).map_err(|_| SyncError::GetUpstreamBranchNameFailed)?;
  let remote_name = repo.branch_remote_name(&branch_upstream_name).map_err(|_| SyncError::GetRemoteBranchNameFailed)?;

  // create request review branch for patch
  let (branch, ps_id) = ps::plumbing::branch::branch(&repo, patch_index).map_err(|e| SyncError::CreateRrBranchFailed(e))?;

  let branch_ref_name = branch.get().shorthand().ok_or(SyncError::RequestReviewBranchNameMissing)?;
  let rr_branch_name = branch_ref_name.to_string();

  // force push request review branch up to remote
  git::ext_push(true, remote_name.as_str().unwrap(), branch_ref_name, branch_ref_name).map_err(|e| SyncError::ForcePushFailed(e))?;

  // associate the patch to the branch that was created
  state_management::update_patch_state(&repo, &ps_id, |patch_meta_data_option|
    match patch_meta_data_option {
      Some(patch_meta_data) => {
        match patch_meta_data.state {
          state_management::PatchState::Published(ref _branch_name) => patch_meta_data.clone(),
          state_management::PatchState::RequestedReview(ref _branch_name) => patch_meta_data.clone(),
          _ => {
            state_management::Patch {
              patch_id: ps_id,
              state: state_management::PatchState::PushedToRemote(rr_branch_name)
            }
          }
        }
      },
      None => {
        state_management::Patch {
          patch_id: ps_id,
          state: state_management::PatchState::PushedToRemote(rr_branch_name)
        }
      }
    }
  ).map_err(|e| SyncError::StorePatchStateFailed(e))?;

  Ok(())
}